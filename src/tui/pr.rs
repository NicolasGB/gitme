use std::sync::{Arc, RwLock};

use octocrab::{
    Page,
    params::{Direction, State},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Style, Stylize},
    text::Line,
    widgets::{Block, HighlightSpacing, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct PullRequestWidget {
    config: Config,
    state: Arc<RwLock<PullRequestListState>>,
}

#[derive(Debug, Default)]
struct PullRequestListState {
    pull_requests: Vec<PullRequest>,
    loading_state: LoadingState,
    table_state: TableState,
}

#[derive(Debug, Clone)]
struct PullRequest {
    id: String,
    title: String,
    url: String,
    repo: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error(String),
}

impl PullRequestWidget {
    pub(crate) fn new(config: Config) -> Self {
        Self {
            config,
            state: Default::default(),
        }
    }

    pub(crate) fn run(&self) {
        self.config.repositories.iter().for_each(|r| {
            let this = self.clone(); // clone the widget to pass to the background task
            tokio::spawn(this.fetch_pulls(r.owner.clone(), r.name.clone()));
        });
    }

    async fn fetch_pulls(self, owner: String, repo: String) {
        self.set_loading_state(LoadingState::Loading);

        let pulls = octocrab::instance()
            .pulls(owner, repo)
            .list()
            .state(State::Open)
            .direction(Direction::Descending)
            .send()
            .await;

        match pulls {
            Ok(page) => self.on_load(&page),
            Err(err) => self.on_err(&err),
        }
    }

    fn on_load(&self, page: &Page<OctoPullRequest>) {
        let prs = page
            .items
            .iter()
            // Get only prs where my review was requested
            .filter(|pr| {
                if let Some(reviewers) = &pr.requested_reviewers {
                    if let Some(username) = &self.config.username {
                        return reviewers.iter().any(|e| e.login == *username);
                    }
                }
                false
            })
            .map(Into::into);

        let mut state = self.state.write().unwrap();
        state.loading_state = LoadingState::Loaded;

        state.pull_requests.extend(prs);
        if !state.pull_requests.is_empty() {
            state.table_state.select(Some(0));
        }
    }

    fn on_err(&self, err: &octocrab::Error) {
        self.set_loading_state(LoadingState::Error(err.to_string()));
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    pub fn scroll_down(&self) {
        self.state.write().unwrap().table_state.scroll_down_by(1);
    }

    pub fn scroll_up(&self) {
        self.state.write().unwrap().table_state.scroll_up_by(1);
    }

    pub fn open(&self) {
        let lock = self.state.write().unwrap();
        if let Some(selected) = lock.table_state.selected() {
            // Safe to unwrap since it won't fail
            let pr = lock.pull_requests.get(selected).unwrap();
            open::that(&pr.url).unwrap();
        }
    }
}

impl Widget for &PullRequestWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = self.state.write().unwrap();

        // Create a more stylish loading state indicator
        let loading_indicator = match state.loading_state {
            LoadingState::Loading => "Loading...".yellow(),
            LoadingState::Loaded => "✓ Ready".green(),
            LoadingState::Error(ref e) => {
                let error_msg = format!("⚠ Error: {}", e);
                // Split long errors into multiple lines if needed
                if error_msg.len() > area.width as usize {
                    format!("⚠ Error: {:.1$}...", e, area.width as usize - 15).red()
                } else {
                    error_msg.red()
                }
            }
            LoadingState::Idle => "Idle".dark_gray(),
        };
        let loading_state = Line::from(loading_indicator).right_aligned();

        // Enhanced block with subtle styling
        let block = Block::bordered()
            .title("📋 Pull Requests".bold())
            .title(loading_state)
            .title_bottom("↑↓ to scroll • q to quit".dark_gray())
            .border_type(ratatui::widgets::BorderType::Rounded);

        // Table with pull requests
        let rows = state.pull_requests.iter();
        let widths = [
            Constraint::Length(6),  // Slightly wider for ID
            Constraint::Fill(1),    // Title takes most space
            Constraint::Length(50), // Fixed width for repo
        ];

        let table = Table::new(rows, widths)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol("▶ ") // Using a triangle as cursor
            .header(
                Row::new(vec!["ID", "Title", "Repository"])
                    .style(Style::new().bold().yellow())
                    .bottom_margin(1),
            )
            .row_highlight_style(
                Style::default()
                    .bg(ratatui::style::Color::Blue)
                    .fg(ratatui::style::Color::Black)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

type OctoPullRequest = octocrab::models::pulls::PullRequest;

impl From<&OctoPullRequest> for PullRequest {
    fn from(pr: &OctoPullRequest) -> Self {
        Self {
            id: pr.number.to_string(),
            title: pr.title.as_ref().unwrap().to_string(),
            url: pr
                .html_url
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            repo: pr.base.repo.as_ref().unwrap().name.clone(),
        }
    }
}

impl From<&PullRequest> for Row<'_> {
    fn from(pr: &PullRequest) -> Self {
        let pr = pr.clone();
        Row::new(vec![pr.id, pr.title, pr.repo])
    }
}
