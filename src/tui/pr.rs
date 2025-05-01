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
    widgets::{Block, Cell, Paragraph, Row, Table, Widget, Wrap},
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

#[derive(Debug, Clone, PartialEq)]
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
    ("f", "Refetch pulls"),
    ("Space", "Toggle Expand"), // Assuming space toggles expand based on pr_list_state
    ("z", "Expand All"),
    ("c", "Collapse All"),
    ("r", "Review PR"),
    ("o", "Open in Browser"),
    ("q", "Quit"),
];

impl PullRequestWidget {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Default::default(),
        }
    }

    pub fn run(&self) {
        self.refresh_pull_requests();
    }

    async fn fetch_pulls(
        app_state: Arc<RwLock<AppState>>,
        username: Option<String>,
        owner: String,
        repo: String,
    ) {
        Self::set_loading_state(&app_state, LoadingState::Loading);

        let pulls = octocrab::instance()
            .pulls(owner, repo)
            .list()
            .state(State::Open)
            .direction(Direction::Descending)
            .send()
            .await;

        match pulls {
            Ok(page) => Self::on_load(&app_state, &username, &page),
            Err(err) => Self::on_err(&app_state, &err),
        }
    }

    // On a load of prs received, pushes them in their corresponding map entry in the prs state
    fn on_load(
        app_state: &Arc<RwLock<AppState>>,
        username: &Option<String>,
        page: &Page<OctoPullRequest>,
    ) {
        // List the pull requests filter them by the user that has to review them
        let prs_review: Vec<PullRequest> = page
            .items
            .iter()
            // Get only prs where my review was requested
            .filter(|pr| {
                if let Some(reviewers) = &pr.requested_reviewers {
                    if let Some(username) = username {
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
                    if let Some(username) = username {
                        return assignees.iter().any(|e| e.login == *username);
                    }
                }
                false
            })
            .map(Into::into)
            .collect();

        let mut state = app_state.write().unwrap();
        state.loading_state = LoadingState::Loaded;

        // Group the prs by repository to be able to better handle them later on in a tree view
        for pr in prs_review {
            let repo = state
                .review_prs
                .grouped_prs
                .entry(pr.repo.clone())
                .or_default();

            if !repo.contains(&pr) {
                repo.push(pr);
            }
        }

        // If the map is not empty, and theres not a previously selected state
        if !state.review_prs.grouped_prs.is_empty()
            && state.review_prs.table_state.selected().is_none()
        {
            state.review_prs.table_state.select(Some(0));
        }

        for pr in prs_assignee {
            let repo = state
                .assignee_prs
                .grouped_prs
                .entry(pr.repo.clone())
                .or_default();

            if !repo.contains(&pr) {
                repo.push(pr);
            }
        }

        // If the map is not empty, and theres not a previously selected state
        if !state.assignee_prs.grouped_prs.is_empty()
            && state.assignee_prs.table_state.selected().is_none()
        {
            state.assignee_prs.table_state.select(Some(0));
        }
    }

    fn on_err(app_state: &Arc<RwLock<AppState>>, err: &octocrab::Error) {
        let error_message = match err {
            octocrab::Error::GitHub { source, .. } => source.message.clone(),
            // Fallback to display
            _ => format!("{}", err),
        };

        Self::set_loading_state(app_state, LoadingState::Error(error_message));
    }

    fn set_loading_state(app_state: &Arc<RwLock<AppState>>, state: LoadingState) {
        app_state.write().unwrap().loading_state = state;
    }
}

// Eventful functions
impl PullRequestWidget {
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

    /// Calls the github api again and updates the prs
    pub fn refresh_pull_requests(&self) {
        self.config.repositories.iter().for_each(|r| {
            let state = self.state.clone(); // clone the widget to pass to the background task
            let username = self.config.username.clone();
            let owner = r.owner.clone();
            let repo = r.name.clone();
            tokio::spawn(Self::fetch_pulls(state, username, owner, repo));
        });
    }
}

impl Widget for &PullRequestWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 1. Calculate Layout (could be a helper function)
        let (prs_area, details_area, footer_area) = self.calculate_main_layout(area);
        let (details_title_area, details_body_area) = self.calculate_details_layout(details_area);

        // 2. Acquire Lock
        let mut state = self.state.write().unwrap();

        // 3. Render Footer
        self.render_footer(&state.loading_state, footer_area, buf);

        // 4. Render Main Panels using state
        self.render_pr_list_panel(&mut state, prs_area, buf);
        self.render_details_panel(&state.details, details_title_area, details_body_area, buf);

        // 5. Render Popups if needed
        if state.show_help {
            self.render_help_popup(area, buf); // area is the full screen for centering
        }
        if let LoadingState::Error(ref msg) = state.loading_state {
            self.render_error_popup(msg, area, buf); // area is the full screen for centering
        }
    }
}

// Render related functions
impl PullRequestWidget {
    // --- Helper render functions ---
    fn calculate_main_layout(&self, area: Rect) -> (Rect, Rect, Rect) {
        let base_layout =
            Layout::vertical([Constraint::Percentage(95), Constraint::Min(3)]).split(area);
        let prs_layout =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(base_layout[0]);
        (prs_layout[0], prs_layout[1], base_layout[1])
    }

    fn calculate_details_layout(&self, details_area: Rect) -> (Rect, Rect) {
        let details_layout =
            Layout::vertical([Constraint::Max(3), Constraint::Min(10)]).split(details_area);
        (details_layout[0], details_layout[1])
    }

    fn render_pr_list_panel(&self, state: &mut AppState, area: Rect, buf: &mut Buffer) {
        // Build title line based on state.active_panel
        let review_requested = if state.active_panel == ActivePanel::PullRequestsToReview {
            "Review Requested".bold()
        } else {
            "Review Requested".dark_gray()
        };
        let my_prs = if state.active_panel == ActivePanel::MyPullRequests {
            "My Pull Requests ".bold()
        } else {
            "My Pull Requests ".dark_gray()
        };
        let title_line = Line::from(vec!["📋 ".into(), review_requested, " - ".into(), my_prs]);

        let prs_block = Block::default()
            .title(title_line)
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .border_type(ratatui::widgets::BorderType::Rounded);

        match state.active_panel {
            ActivePanel::PullRequestsToReview => {
                state.review_prs.render_table(prs_block, area, buf);
            }
            ActivePanel::MyPullRequests => {
                state.assignee_prs.render_table(prs_block, area, buf);
            }
        };
    }

    fn render_details_panel(
        &self,
        details_state: &PullRequestsDetailsState,
        title_area: Rect,
        body_area: Rect,
        buf: &mut Buffer,
    ) {
        let title_block = Block::default()
            .title("Title")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let details_block = Block::default()
            .title("Details")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        if let Some(pr_details) = &details_state.pr_details {
            Paragraph::new(&*pr_details.title)
                .block(title_block)
                .wrap(Wrap { trim: true })
                .render(title_area, buf);

            let body_content = tui_markdown::from_str(&pr_details.body);
            Paragraph::new(body_content)
                .block(details_block)
                .wrap(Wrap { trim: true })
                .render(body_area, buf);
        } else {
            title_block.render(title_area, buf);
            details_block.render(body_area, buf);
        }
    }

    fn render_footer(&self, loading_state: &LoadingState, area: Rect, buf: &mut Buffer) {
        let loading_state = match loading_state {
            LoadingState::Loading => String::from("Loading...").yellow(),
            _ => String::from("").white(),
        };
        let bottom_box = Block::default()
            .title("Help")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let help_line = Line::styled(
            format!(
                "{loading_state} Scroll: ↑↓,j/k • Switch: TAB • Review: r • Keybindings: ? • Quit: q"
            ),
            Color::Green, // Consider theming
        );

        let bottom_inner = bottom_box.inner(area);
        bottom_box.render(area, buf);
        help_line.render(bottom_inner, buf);
    }

    fn render_help_popup(&self, screen_area: Rect, buf: &mut Buffer) {
        let rows = KEYBINDINGS.iter().map(|(key, action)| {
            Row::new(vec![
                Cell::from(key.to_string()).style(Style::default().fg(Color::Cyan)), // Theming
                Cell::from(action.to_string()).style(Style::default().fg(Color::Green)), // Theming
            ])
        });

        let area = centered_rect(screen_area, 20, 15, 20, 11); // Use the full screen_area for centering
        let popup_block = Block::default()
            .title(" Keybindings ")
            .title_bottom(" Esc to close ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::LightCyan)); // Theming

        let help_table = Table::new(rows, [Constraint::Length(10), Constraint::Min(15)])
            .block(popup_block)
            .column_spacing(2);

        ratatui::widgets::Clear.render(area, buf);
        ratatui::prelude::Widget::render(help_table, area, buf);
    }

    fn render_error_popup(&self, err_msg: &str, screen_area: Rect, buf: &mut Buffer) {
        let popup_block = Block::default()
            .title(" Error ")
            .title_bottom(" q to quit ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Red)); // Theming

        let error_paragraph = Paragraph::new(err_msg.red()) // Use reference, apply style
            .block(popup_block)
            .centered()
            .wrap(Wrap { trim: true });

        let area = centered_rect(screen_area, 30, 20, 30, 10); // Use the full screen_area for centering
        ratatui::widgets::Clear.render(area, buf);
        error_paragraph.render(area, buf);
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
