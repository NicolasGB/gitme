use std::collections::HashMap;

use crate::tui::utils;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{
        Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap,
    },
};

use super::{Profile, PullRequest};

#[derive(Debug, Default, PartialOrd, PartialEq)]
enum ActivePanel {
    #[default]
    Body,
    Reviews,
}

#[derive(Debug, Default)]
pub struct PullRequestsDetailsState {
    active_panel: ActivePanel,
    pub pr_details: Option<PullRequest>,
    pub body_scroll: u16,
    pub scrollbar_state: ScrollbarState,
    pub cached_authors: HashMap<String, Profile>,
}

impl PullRequestsDetailsState {
    fn calculate_details_layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let details_layout =
            Layout::vertical([Constraint::Max(3), Constraint::Min(10), Constraint::Max(3)])
                .split(area);
        (details_layout[0], details_layout[1], details_layout[2])
    }

    pub fn next_tab(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Body => ActivePanel::Reviews,
            ActivePanel::Reviews => ActivePanel::Body,
        };
    }

    pub fn set_pull_request(&mut self, pr: Option<PullRequest>) {
        self.pr_details = pr;
        self.body_scroll = 0;
        self.scrollbar_state = ScrollbarState::default();
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let (title_area, tab_area, footer_area) = self.calculate_details_layout(area);

        let title_block = utils::block_with_title("Title");
        let details_title = match self.active_panel {
            ActivePanel::Body => Line::from(vec!["Details".bold(), " - Reviews".dark_gray()]),
            ActivePanel::Reviews => Line::from(vec!["Details - ".dark_gray(), "Reviews".bold()]),
        };
        let details_block = utils::block_with_title(details_title);

        // Split the footer into different blocks
        let footer_layout = Layout::horizontal([
            Constraint::Min(30),
            Constraint::Max(13),
            Constraint::Max(13),
        ])
        .split(footer_area);

        let author_block = utils::block_with_title("Author");
        let mergeable_block = utils::block_with_title("Mergeable");
        let rebaseable_block = utils::block_with_title("Rebaseable");

        let get_status_span = |value: bool| {
            if value {
                Span::styled("Yes", Style::default().fg(Color::Green))
            } else {
                Span::styled("No", Style::default().fg(Color::Red))
            }
        };

        if let Some(pr_details) = &self.pr_details {
            Paragraph::new(&*pr_details.title)
                .block(title_block)
                .wrap(Wrap { trim: true })
                .render(title_area, buf);

            // let body_content = tui_markdown::from_str(&pr_details.body);
            let body_inner = details_block.inner(tab_area);
            let body_paragraph = Paragraph::new(&*pr_details.body)
                .block(details_block)
                .wrap(Wrap { trim: true })
                .scroll((self.body_scroll, 0));

            body_paragraph.render(tab_area, buf);

            // Check if there needs to be a scrollbar displayed meaning that the total lines
            // wrapped  are greater than the inner body viewport
            let wrapped_lines = textwrap::wrap(&pr_details.body, body_inner.width as usize);
            let total_lines_after_wrapping = wrapped_lines.len();
            let viewport_height = body_inner.height as usize;

            if total_lines_after_wrapping > viewport_height {
                self.scrollbar_state = self
                    .scrollbar_state
                    .content_length(total_lines_after_wrapping)
                    .viewport_content_length(viewport_height);

                Scrollbar::new(ScrollbarOrientation::VerticalRight).render(
                    tab_area,
                    buf,
                    &mut self.scrollbar_state,
                );
            }

            // If we have the author in the cache, get it frm there
            let author = if let Some(prof) = self.cached_authors.get(&pr_details.author) {
                format!("{} ({})", prof.name, prof.login)
            } else {
                pr_details.author.clone()
            };

            Paragraph::new(author)
                .block(author_block)
                .wrap(Wrap { trim: true })
                .render(footer_layout[0], buf);

            let mergeable_span = get_status_span(pr_details.mergeable);
            Paragraph::new(mergeable_span)
                .block(mergeable_block)
                .wrap(Wrap { trim: true })
                .render(footer_layout[1], buf);

            let rebaseable_span = get_status_span(pr_details.rebaseable);
            Paragraph::new(rebaseable_span)
                .block(rebaseable_block)
                .wrap(Wrap { trim: true })
                .render(footer_layout[2], buf);
        } else {
            // Render the empty blocks
            title_block.render(title_area, buf);
            details_block.render(tab_area, buf);
            author_block.render(footer_layout[0], buf);
            mergeable_block.render(footer_layout[1], buf);
            rebaseable_block.render(footer_layout[2], buf);
        }
    }
}
