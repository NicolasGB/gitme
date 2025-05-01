mod pr_list_state;

use std::{
    process::Command,
    sync::{Arc, RwLock},
};

use octocrab::{
    Page,
    params::{Direction, State},
};
use pr_list_state::PullRequestsListState;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Cell, Paragraph, Row, StatefulWidget, Table, Widget, Wrap},
};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct PullRequestWidget {
    config: Config,
    state: Arc<RwLock<AppState>>,
}

#[derive(Debug, Default)]
struct AppState {
    active_panel: ActivePanel,

    review_prs: PullRequestsListState,
    assignee_prs: PullRequestsListState,

    details: PullRequestsDetailsState,

    loading_state: LoadingState,
    show_help: bool,
}

#[derive(Debug, Default)]
struct PullRequestsDetailsState {
    pr_details: Option<PullRequest>,
}

#[derive(Debug, Clone)]
struct PullRequest {
    id: String,
    title: String,
    url: String,
    repo: String,
    body: String,
    is_draft: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd)]
enum ActivePanel {
    #[default]
    PullRequestsToReview,
    MyPullRequests,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error(String),
}

/// Helper function to create a centered rect using percentages.
/// Ensures the rectangle has a minimum size, taking up more relative space
/// if the provided area is small.
fn centered_rect(
    area: Rect,
    percent_x: u16,
    percent_y: u16,
    min_width: u16,
    min_height: u16,
) -> Rect {
    // Determine the final dimensions, ensuring they are at least the minimum
    // and do not exceed the available area dimensions.
    let target_width = (area.width as f32 * percent_x as f32 / 100.0) as u16;
    let target_height = (area.height as f32 * percent_y as f32 / 100.0) as u16;

    let final_width = target_width.max(min_width).min(area.width);
    let final_height = target_height.max(min_height).min(area.height);

    let vertical_layout = Layout::vertical([Constraint::Length(final_height)]).flex(Flex::Center);
    let horizontal_layout =
        Layout::horizontal([Constraint::Length(final_width)]).flex(Flex::Center);

    let [centered_vertically] = vertical_layout.areas(area);
    let [final_area] = horizontal_layout.areas(centered_vertically);

    final_area
}

const KEYBINDINGS: &[(&str, &str)] = &[
    ("↑↓, j/k", "Scroll List"),
    ("TAB", "Switch Panel"),
    ("Space", "Toggle Expand"), // Assuming space toggles expand based on pr_list_state
    ("z", "Expand All"),
    ("c", "Collapse All"),
    ("r", "Review PR"),
    ("o", "Open in Browser"),
    ("q", "Quit"),
];

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

    // On a load of prs received, pushes them in their corresponding map entry in the prs state
    fn on_load(&self, page: &Page<OctoPullRequest>) {
        // List the pull requests filter them by the user that has to review them
        let prs_review: Vec<PullRequest> = page
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
            .map(Into::into)
            .collect();

        let prs_assignee: Vec<PullRequest> = page
            .items
            .iter()
            .filter(|pr| {
                if let Some(assignees) = &pr.assignees {
                    if let Some(username) = &self.config.username {
                        return assignees.iter().any(|e| e.login == *username);
                    }
                }
                false
            })
            .map(Into::into)
            .collect();

        let mut state = self.state.write().unwrap();
        state.loading_state = LoadingState::Loaded;

        // Group the prs by repository to be able to better handle them later on in a tree view
        for pr in prs_review {
            state
                .review_prs
                .grouped_prs
                .entry(pr.repo.clone())
                .or_default()
                .push(pr);
        }

        // If the map is not empty, and theres not a previously selected state
        if !state.review_prs.grouped_prs.is_empty()
            && state.review_prs.table_state.selected().is_none()
        {
            state.review_prs.table_state.select(Some(0));
        }

        for pr in prs_assignee {
            state
                .assignee_prs
                .grouped_prs
                .entry(pr.repo.clone())
                .or_default()
                .push(pr);
        }

        // If the map is not empty, and theres not a previously selected state
        if !state.assignee_prs.grouped_prs.is_empty()
            && state.assignee_prs.table_state.selected().is_none()
        {
            state.assignee_prs.table_state.select(Some(0));
        }
    }

    fn on_err(&self, err: &octocrab::Error) {
        let error_message = match err {
            octocrab::Error::GitHub { source, .. } => source.message.clone(),
            // Fallback to display
            _ => format!("{}", err),
        };

        self.set_loading_state(LoadingState::Error(error_message));
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    pub fn scroll_down(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = match state.active_panel {
            ActivePanel::PullRequestsToReview => &mut state.review_prs,
            ActivePanel::MyPullRequests => &mut state.assignee_prs,
        };
        prs_state.scroll_down();

        // If a pr is selected make it available in the details
        state.details.pr_details = prs_state.find_selected().cloned();
    }

    pub fn scroll_up(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = match state.active_panel {
            ActivePanel::PullRequestsToReview => &mut state.review_prs,
            ActivePanel::MyPullRequests => &mut state.assignee_prs,
        };
        prs_state.scroll_up();

        state.details.pr_details = prs_state.find_selected().cloned();
    }

    pub fn expand_all(&self) {
        let mut state = self.state.write().unwrap();

        let repos: Vec<String> = state.review_prs.grouped_prs.keys().cloned().collect();

        repos.iter().for_each(|repo| {
            state.review_prs.expanded_repos.insert(repo.clone());
        });
    }

    pub fn contract_all(&self) {
        let mut state = self.state.write().unwrap();

        state.review_prs.expanded_repos.clear();
    }

    pub fn toggle_expand(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = match state.active_panel {
            ActivePanel::PullRequestsToReview => &mut state.review_prs,
            ActivePanel::MyPullRequests => &mut state.assignee_prs,
        };
        prs_state.toggle_expand();
    }

    pub fn next_tab(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = match state.active_panel {
            ActivePanel::PullRequestsToReview => {
                state.active_panel = ActivePanel::MyPullRequests;
                &mut state.assignee_prs
            }
            ActivePanel::MyPullRequests => {
                state.active_panel = ActivePanel::PullRequestsToReview;
                &mut state.review_prs
            }
        };

        state.details.pr_details = prs_state.find_selected().cloned();
    }

    pub fn open(&self) {
        let state = self.state.read().unwrap();
        let prs_state = match state.active_panel {
            ActivePanel::PullRequestsToReview => &state.review_prs,
            ActivePanel::MyPullRequests => &state.assignee_prs,
        };

        if let Some(pr) = prs_state.find_selected() {
            open::that(pr.url.clone()).unwrap();
        }
    }

    pub fn review(&self) {
        let state = self.state.read().unwrap();

        // Only available with reviewable prs
        if let ActivePanel::PullRequestsToReview = state.active_panel {
            if let Some(pr) = state.review_prs.find_selected() {
                // TODO: handle missing paths or config repo
                if let Some(config_repo) =
                    self.config.repositories.iter().find(|r| r.name == pr.repo)
                {
                    let cmd = self.config.command.clone().unwrap_or_else(|| {
                        std::env::var("TERMINAL").unwrap_or_else(|_| "ghostty".to_string())
                    });

                    if let Some(path) = &config_repo.system_path {
                        let args = self.config.command_args.clone();
                        let path = path.clone();
                        std::thread::spawn(move || {
                            // First change to the target directory
                            std::env::set_current_dir(&path).unwrap_or_else(|e| {
                                eprintln!("Failed to change directory: {}", e);
                            });

                            let mut cmd = Command::new(cmd);
                            for arg in args.iter() {
                                cmd.arg(arg);
                            }
                            cmd.output()
                        });
                    }
                }
            }
        }
    }

    pub fn toggle_help(&self) {
        let mut state = self.state.write().unwrap();
        state.show_help = !state.show_help
    }

    pub fn help_open(&self) -> bool {
        self.state.read().unwrap().show_help
    }
}

impl Widget for &PullRequestWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let base_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![Constraint::Percentage(95), Constraint::Min(3)])
            .split(area);

        let prs_layout = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(base_layout[0]);

        // Split the details into 3 zones, title, details, comments
        let details_layout = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Max(3), Constraint::Min(10)])
            .split(prs_layout[1]);

        // Now that all the layout is ready and the blocks too, we acquire the lock
        let mut state = self.state.write().unwrap();

        // Build the text with color according to active panel
        let review_requested = if let ActivePanel::PullRequestsToReview = state.active_panel {
            "Review Requested".bold()
        } else {
            "Review Requested".dark_gray()
        };

        let my_prs = if let ActivePanel::MyPullRequests = state.active_panel {
            "My Pull Requests ".bold()
        } else {
            "My Pull Requests ".dark_gray()
        };

        let title_spans = vec!["📋 ".into(), review_requested, " - ".into(), my_prs];
        let title_line = ratatui::text::Line::from(title_spans);

        // Create two simple boxes to visualize the splits
        let prs_block = Block::default()
            .title(title_line)
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let title_block = Block::default()
            .title("Title")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let details_block = Block::default()
            .title("Details")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let bottom_box = Block::default()
            .title("Help")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let loading_state = match state.loading_state {
            LoadingState::Loading => String::from("Loading...").yellow(),
            _ => String::from("").white(),
        };

        let help_line = Line::styled(
            format!(
                "{loading_state} Scroll: ↑↓,j/k • Switch: TAB • Review: r • Keybindings: ? • Quit: q"
            ),
            Color::Green,
        );

        // Draw the table
        let columns = [Constraint::Fill(1)];
        // Based on the state render the rows and get the table state to render
        let (rows, table_state) = match state.active_panel {
            ActivePanel::PullRequestsToReview => {
                let rows = PullRequest::render_table_based_on_state(&state.review_prs);
                (rows, &mut state.review_prs.table_state)
            }
            ActivePanel::MyPullRequests => {
                let rows = PullRequest::render_table_based_on_state(&state.assignee_prs);
                (rows, &mut state.assignee_prs.table_state)
            }
        };

        let table = Table::new(rows, columns)
            .block(prs_block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::Rgb(76, 55, 67)) // #4c3743
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );

        // Render the table as a stateful widget
        StatefulWidget::render(table, prs_layout[0], buf, table_state);

        // If details inside, get the area and render it
        if let Some(pr_details) = &state.details.pr_details {
            // Title of the pr
            Paragraph::new(pr_details.title.clone())
                .block(title_block)
                .wrap(Wrap { trim: true })
                .render(details_layout[0], buf);

            // Body of the PR
            Paragraph::new(tui_markdown::from_str(&pr_details.body.clone()))
                .block(details_block)
                .wrap(Wrap { trim: true })
                .render(details_layout[1], buf);
        } else {
            // Render simply the box without anything
            title_block.render(details_layout[0], buf);
            details_block.render(details_layout[1], buf);
        }
        // Render bottom part
        // Here we get the inner of the bottom from it's layout, then we render the other two
        let bottom_inner = bottom_box.inner(base_layout[1]);
        bottom_box.render(base_layout[1], buf);
        help_line.render(bottom_inner, buf);

        // Render help popup if required
        if state.show_help {
            let rows = KEYBINDINGS.iter().map(|(key, action)| {
                Row::new(vec![
                    Cell::from(key.to_string()).style(Style::default().fg(Color::Cyan)),
                    Cell::from(action.to_string()).style(Style::default().fg(Color::Green)),
                ])
            });

            let area = centered_rect(area, 20, 15, 20, 10);
            let popup_block = Block::default()
                .title(" Keybindings ")
                .title_bottom(" Esc to close ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::LightCyan));

            // Layout the table rows
            let help_table = Table::new(rows, [Constraint::Length(10), Constraint::Min(15)])
                .block(popup_block)
                .column_spacing(2);

            // Clear the area before drawing the popup
            ratatui::widgets::Clear.render(area, buf);

            // Render explicitly to avoid confusion with stateless and stateful table
            ratatui::prelude::Widget::render(help_table, area, buf);
        }

        // Render error popup if needed
        if let LoadingState::Error(ref err_msg) = state.loading_state {
            let popup_block = Block::default()
                .title(" Error ")
                .title_bottom(" q to quit ")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Red));

            let error_paragraph = Paragraph::new(err_msg.clone().red())
                .block(popup_block)
                .centered()
                .wrap(Wrap { trim: true });

            let area = centered_rect(area, 30, 20, 30, 10);
            // Clear the area before drawing the popup
            ratatui::widgets::Clear.render(area, buf);
            error_paragraph.render(area, buf);
        }
    }
}

impl PullRequest {
    fn render_table_based_on_state<'a>(
        prs_state: &PullRequestsListState,
    ) -> Vec<ratatui::widgets::Row<'a>> {
        let mut rows = Vec::new();
        for (group, prs) in prs_state.grouped_prs.iter() {
            let expanded = prs_state.expanded_repos.contains(group);
            if expanded {
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
            } else {
                rows.push(Row::new([format!("▶ {} ({})", group, prs.len())]));
            }
        }
        rows
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
            body: pr.body.as_ref().cloned().unwrap_or_default(),
            is_draft: pr.draft.unwrap_or_default(),
        }
    }
}

impl From<&PullRequest> for Row<'_> {
    fn from(pr: &PullRequest) -> Self {
        let pr = pr.clone();
        Row::new(vec![pr.id, pr.title, pr.repo])
    }
}
