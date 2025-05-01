use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Row, StatefulWidget, Table, TableState},
};

use super::PullRequest;

#[derive(Debug, Default)]
pub struct PullRequestsListState {
    pub grouped_prs: std::collections::BTreeMap<String, Vec<PullRequest>>,
    pub table_state: TableState,
}

impl PullRequestsListState {
    pub fn scroll_down(&mut self) {
        // Calculate total number of visible rows
        let total_rows = self
            .grouped_prs
            .iter()
            .fold(0, |acc, (_repo, prs)| acc + 1 + prs.len());
        let current = self.table_state.selected().unwrap_or(0);
        if current + 1 < total_rows {
            self.table_state.scroll_down_by(1);
        }
    }

    pub fn scroll_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }

    pub fn jump_up(&mut self) {
        self.table_state.scroll_up_by(5);
    }

    pub fn jump_down(&mut self) {
        let total_rows = self
            .grouped_prs
            .iter()
            .fold(0, |acc, (_repo, prs)| acc + 1 + prs.len());
        let current = self.table_state.selected().unwrap_or(0);
        if current + 5 > total_rows {
            let last_index = total_rows.saturating_sub(1);
            self.table_state.select(Some(last_index));
        } else {
            self.table_state.scroll_down_by(5);
        }
    }

    pub fn find_selected(&self) -> Option<&PullRequest> {
        if let Some(index) = self.table_state.selected() {
            if let Some(pr) = self.find_by_index(index) {
                return Some(pr);
            }
        }

        None
    }

    fn find_by_index(&self, index: usize) -> Option<&PullRequest> {
        let mut current_index = 0;

        for (_repo, prs) in self.grouped_prs.iter() {
            if current_index == index {
                // Here we're returning none, since it matches a header row
                return None;
            }
            // Increment for the header row of the group
            current_index += 1;

            for pr in prs.iter() {
                if index == current_index {
                    return Some(pr);
                }
                // Increment the just seen pr
                current_index += 1;
            }
        }
        None
    }

    pub(crate) fn render_table(&mut self, block: Block, area: Rect, buf: &mut Buffer) {
        let mut rows = Vec::new();
        for (group, prs) in self.grouped_prs.iter() {
            rows.push(Row::new([format!("▼ {} ({})", group, prs.len())]));
            let prs_len = prs.len();
            prs.iter().enumerate().for_each(|(i, pr)| {
                let mut prefix = "├─";
                if i == prs_len - 1 {
                    prefix = "└─";
                }
                rows.push(Row::new([format!(
                    "  {} {} #{} - {}",
                    prefix,
                    if pr.is_draft { "📝" } else { "" },
                    pr.id,
                    pr.title
                )]));
            });
        }

        // Build the table and return it
        let t = Table::new(rows, [ratatui::layout::Constraint::Fill(1)])
            .block(block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::Rgb(76, 55, 67)) // #4c3743
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );

        StatefulWidget::render(t, area, buf, &mut self.table_state);
    }
}
