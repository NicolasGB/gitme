mod pr_details_state;
mod pr_list_state;

use std::{
    process::Command,
    sync::{Arc, RwLock},
};

use crossterm::event::Event;
use octocrab::{
    Page,
    models::UserProfile,
    params::{Direction, State},
};
use pr_details_state::PullRequestsDetailsState;
use pr_list_state::PullRequestsListState;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Cell, Paragraph, Row, Table, Widget, Wrap},
};
use tokio::task::JoinSet;
use tui_input::{Input, backend::crossterm::EventHandler};

use crate::config::Config;

use super::utils;

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

    searching: bool,
    search: Input,
    cursor_position: Option<Position>,
}

#[derive(Debug, Clone, PartialEq)]
struct PullRequest {
    id: String,
    title: String,
    url: String,
    repo: String,
    body: String,
    author: String,
    is_draft: bool,
    mergeable: bool,
    rebaseable: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Profile {
    id: String,
    login: String,
    name: String,
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

const KEYBINDINGS: &[(&str, &str)] = &[
    ("↑↓, j/k", "Scroll List"),
    ("n", "Next repository"),
    ("p", "Previous repository"),
    ("Ctrl+d/u", "Scroll Details"),
    ("TAB", "Switch Panel"),
    ("/", "Search"),
    ("f", "Refetch pulls"),
    ("r", "Review PR"),
    ("o", "Open in Browser"),
    ("q", "Quit"),
];

const DETAILS_SCROLL_INCREMENT: u16 = 3;

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
        Self::set_loading_state(Arc::clone(&app_state), LoadingState::Loading);

        let pulls = octocrab::instance()
            .pulls(&owner, &repo)
            .list()
            .state(State::Open)
            .direction(Direction::Descending)
            .send()
            .await;

        match pulls {
            Ok(page) => Self::on_load(app_state, username.as_ref(), &page, owner, repo).await,
            Err(err) => Self::on_err(app_state, &err),
        }
    }

    // On a load of prs received, pushes them in their corresponding map entry in the prs state
    async fn on_load(
        app_state: Arc<RwLock<AppState>>,
        username: Option<&String>,
        page: &Page<OctoPullRequest>,
        owner: String,
        repo: String,
    ) {
        let mut prs_review = vec![];
        let mut prs_assignee = vec![];
        let mut reviews_set = JoinSet::new();
        let mut author_set = JoinSet::new();

        for pr in page.items.iter() {
            // Transform the pr to our domain
            let pr_to_push: PullRequest = pr.into();

            // Check if the author of this pr is already in cache or we need to fetch it
            {
                let state = app_state.read().unwrap();
                //Add the author from the cached authors
                if let Some(user) = &pr.user {
                    // If the user is not in the cache request it's profile
                    if !state.details.cached_authors.contains_key(&user.login) {
                        let id = user.id;
                        author_set.spawn(async move {
                            let prof: Profile = octocrab::instance()
                                .users_by_id(id)
                                .profile()
                                .await
                                .unwrap()
                                .into();
                            prof
                        });
                    }
                }
            }

            // If an username is set in the config, try and fetch reviews/assignees
            if let Some(username) = username {
                // Check if we are assignee
                if let Some(assignees) = &pr.assignees {
                    if assignees.iter().any(|e| e.login == *username) {
                        prs_assignee.push(pr_to_push);
                        // Would be very weird to be assignee and reviewer
                        // as of now we're gonna skip if we are assignee maybe i'll come back to
                        // this decision at some poit
                        continue;
                    }
                }

                // Check if we are reviewers
                if let Some(reviewers) = &pr.requested_reviewers {
                    // If the reviewer is requested and has not yet been reviewed push the pr
                    if reviewers.iter().any(|e| e.login == *username) {
                        prs_review.push(pr_to_push);
                        // Go to next iteration
                        continue;
                    }
                }

                // Otherwise we might have reviewed already but the pr is still open
                // For reference of the doc:
                // Gets the users or teams whose review is requested for a pull request.
                // Once a requested reviewer submits a review, they are no longer considered a requested reviewer.

                // Therefore we are going to request another endpoint to make sure we are not
                // reviewers of the pr (assuming if we have submited a review we are a reviewer)
                let owner = owner.clone();
                let repo = repo.clone();
                let number = pr.number;
                reviews_set.spawn(async move {
                    (
                        octocrab::instance()
                            .pulls(owner, repo)
                            .list_reviews(number)
                            .send()
                            .await,
                        pr_to_push,
                    )
                });
            }
        }

        for (reviews, pr) in reviews_set.join_all().await {
            match reviews {
                Ok(page) => {
                    if page.items.iter().any(|r| {
                        if let Some(u) = &r.user {
                            if let Some(username) = username {
                                return u.login == *username;
                            }
                        }
                        false
                    }) {
                        // If found append the pr to the reviewwers
                        prs_review.push(pr);
                    }
                }
                // If error set it and return
                Err(err) => return Self::on_err(app_state, &err),
            }
        }

        let mut authors_to_add = vec![];
        for author in author_set.join_all().await {
            authors_to_add.push(author);
        }

        let mut state = app_state.write().unwrap();

        // Push all the authors in the global author cache
        authors_to_add.into_iter().for_each(|a| {
            state.details.cached_authors.insert(a.login.clone(), a);
        });

        // handle review prs
        if !prs_review.is_empty() {
            // Get  the review repo and clear previous entries
            let review_repo = state
                .review_prs
                .grouped_prs
                .entry(repo.clone())
                .or_default();

            review_repo.clear();
            review_repo.extend(prs_review);
        } else {
            let _ = state.review_prs.grouped_prs.remove(&repo);
        }
        // Update the view
        state.review_prs.update_view();

        // If the map is not empty, and theres not a previously selected state
        if !state.review_prs.grouped_prs.is_empty()
            && state.review_prs.table_state.selected().is_none()
        {
            state.review_prs.table_state.select(Some(0));
        }

        if !prs_assignee.is_empty() {
            // Now do the same for assigned
            let assignee_repo = state.assignee_prs.grouped_prs.entry(repo).or_default();
            assignee_repo.clear();
            assignee_repo.extend(prs_assignee);
        } else {
            let _ = state.assignee_prs.grouped_prs.remove(&repo);
        }
        // Update the view
        state.assignee_prs.update_view();

        // If the map is not empty, and theres not a previously selected state
        if !state.assignee_prs.grouped_prs.is_empty()
            && state.assignee_prs.table_state.selected().is_none()
        {
            state.assignee_prs.table_state.select(Some(0));
        }

        state.loading_state = LoadingState::Loaded;
    }

    fn on_err(app_state: Arc<RwLock<AppState>>, err: &octocrab::Error) {
        let error_message = match err {
            octocrab::Error::GitHub { source, .. } => source.message.clone(),
            // Fallback to display
            _ => format!("{}", err),
        };

        Self::set_loading_state(app_state, LoadingState::Error(error_message));
    }

    fn set_loading_state(app_state: Arc<RwLock<AppState>>, state: LoadingState) {
        app_state.write().unwrap().loading_state = state;
    }

    fn get_active_prs_state_mut(state: &mut AppState) -> &mut PullRequestsListState {
        match state.active_panel {
            ActivePanel::PullRequestsToReview => &mut state.review_prs,
            ActivePanel::MyPullRequests => &mut state.assignee_prs,
        }
    }
}

// Eventful functions
impl PullRequestWidget {
    pub fn scroll_down(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);
        prs_state.scroll_down();

        // If a pr is selected make it available in the details
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn scroll_up(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);
        prs_state.scroll_up();
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn jump_up(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);
        prs_state.jump_up();
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn jump_down(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);
        prs_state.jump_down();
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn next_repository(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);

        prs_state.next_repository();
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn previous_repository(&self) {
        let mut state = self.state.write().unwrap();
        let prs_state = Self::get_active_prs_state_mut(&mut state);

        prs_state.previous_repository();
        let pr = prs_state.find_selected().cloned();
        state.details.set_pull_request(pr);
    }

    pub fn scroll_details_down(&self) {
        let mut state = self.state.write().unwrap();
        if state.details.pr_details.is_some() {
            let scroll = state
                .details
                .body_scroll
                .saturating_add(DETAILS_SCROLL_INCREMENT);

            state.details.body_scroll = scroll;
            state.details.scrollbar_state = state.details.scrollbar_state.position(scroll as usize);
        }
    }

    pub fn scroll_details_up(&self) {
        let mut state = self.state.write().unwrap();
        if state.details.pr_details.is_some() {
            let scroll = state
                .details
                .body_scroll
                .saturating_sub(DETAILS_SCROLL_INCREMENT);

            state.details.body_scroll = scroll;
            state.details.scrollbar_state = state.details.scrollbar_state.position(scroll as usize);
        }
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

        let pr = prs_state.find_selected().cloned();
        state.details.pr_details = pr;
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

    pub fn searching(&self) -> bool {
        self.state.read().unwrap().searching
    }

    pub fn cursor_position(&self) -> Option<Position> {
        self.state.read().unwrap().cursor_position
    }

    pub fn toggle_search(&self) {
        let mut state = self.state.write().unwrap();
        state.searching = !state.searching
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

    pub fn clear_search(&self) {
        let mut state = self.state.write().unwrap();
        state.search.reset();
        state.review_prs.clear_filter_query();
        state.assignee_prs.clear_filter_query();
    }

    pub fn handle_search_input(&self, event: &Event) {
        let mut state = self.state.write().unwrap();
        state.search.handle_event(event);

        let value = state.search.value().to_string();

        // We search in BOTH of the lists
        state.review_prs.set_filter_query(Some(value.clone()));
        state.assignee_prs.set_filter_query(Some(value));
    }
}

impl Widget for &PullRequestWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // 1. Calculate Layout
        let (prs_area, details_area, footer_area) = self.calculate_main_layout(area);

        // 2. Acquire Lock
        let mut state = self.state.write().unwrap();

        // 3. Render Footer
        self.render_footer(&mut state, footer_area, buf);

        // 4. Render Main Panels using state
        self.render_pr_list_panel(&mut state, prs_area, buf);
        state.details.render(details_area, buf);

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

        let mut prs_block = utils::block_with_title(title_line);

        // If we're not searching we are focused on the panel block
        if !state.searching {
            prs_block = prs_block.border_style(Style::default().fg(Color::Green));
        }

        match state.active_panel {
            ActivePanel::PullRequestsToReview => {
                state.review_prs.render_table(prs_block, area, buf);
            }
            ActivePanel::MyPullRequests => {
                state.assignee_prs.render_table(prs_block, area, buf);
            }
        };
    }

    fn render_footer(&self, state: &mut AppState, area: Rect, buf: &mut Buffer) {
        // Create the block with common styling first
        let mut bottom_box = Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        // Calculate inner area *before* setting title and potentially border style
        let bottom_inner = bottom_box.inner(area);

        // Determine title, content, and apply conditional styling
        if state.searching || !state.search.value().is_empty() {
            bottom_box = bottom_box
                .title("Search")
                .border_style(Style::default().fg(Color::Green)); // Green border when searching

            // Render the block first to draw borders
            bottom_box.render(area, buf);

            // Calculate scroll for the search input
            let width = bottom_inner.width.max(1) as usize; // Ensure width is at least 1
            let scroll = state.search.visual_scroll(width);
            let input_paragraph = Paragraph::new(state.search.value());

            // Render the search text itself inside the inner area
            input_paragraph.render(bottom_inner, buf);

            // Calculate and store cursor position if searching is active
            if state.searching {
                let cursor_x_offset = (state.search.visual_cursor().max(scroll) - scroll) as u16;
                // Store the calculated absolute position
                state.cursor_position = Some(Position {
                    x: bottom_inner.x + cursor_x_offset, // Absolute X position
                    y: bottom_inner.y,                   // Absolute Y position
                });
            } else {
                // Not actively searching, but might have text, don't show cursor
                state.cursor_position = None;
            }
        } else {
            // Not searching: Render help text
            bottom_box = bottom_box.title("Help"); // Default title

            let bottom_inner_parts =
                Layout::horizontal([Constraint::Percentage(90), Constraint::Min(15)])
                    .split(bottom_inner);

            // Render the block first
            bottom_box.render(area, buf);

            let loading_state = match state.loading_state {
                LoadingState::Loading => "Loading... ".yellow().into_right_aligned_line(),
                LoadingState::Idle | LoadingState::Loaded => {
                    "Loaded ✔  ".green().into_right_aligned_line()
                }
                LoadingState::Error(_) => "Error ✗ ".red().into_right_aligned_line(),
            };

            let help_line = Line::from(
                "Scroll: ↑↓,j/k • Switch: TAB • Review: r • Keybindings: ? • Quit: q".green(),
            );

            // Render help text inside the inner area
            help_line.render(bottom_inner_parts[0], buf);
            loading_state.render(bottom_inner_parts[1], buf);

            // No cursor when showing help
            state.cursor_position = None;
        }
    }

    fn render_help_popup(&self, screen_area: Rect, buf: &mut Buffer) {
        let rows = KEYBINDINGS.iter().map(|(key, action)| {
            Row::new(vec![
                Cell::from(key.to_string()).style(Style::default().fg(Color::Cyan)), // Theming
                Cell::from(action.to_string()).style(Style::default().fg(Color::Green)), // Theming
            ])
        });

        let area = utils::centered_rect(screen_area, 30, 20, 35, 12); // Use the full screen_area for centering
        let popup_block = utils::block_with_title(" Keybindings ")
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
        let popup_block = utils::block_with_title(" Errors ")
            .title_bottom(" q to quit ")
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(Color::Red)); // Theming

        let error_paragraph = Paragraph::new(err_msg.red()) // Use reference, apply style
            .block(popup_block)
            .centered()
            .wrap(Wrap { trim: true });

        let area = utils::centered_rect(screen_area, 30, 20, 30, 10); // Use the full screen_area for centering
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
            author: pr
                .user
                .as_ref()
                .map(|a| {
                    if let Some(email) = &a.email {
                        format!("{} - {}", a.login, email)
                    } else {
                        a.login.clone()
                    }
                })
                .unwrap_or_default(),
            mergeable: pr.mergeable.unwrap_or_default(),
            rebaseable: pr.rebaseable.unwrap_or_default(),
        }
    }
}

impl From<UserProfile> for Profile {
    fn from(prof: UserProfile) -> Self {
        Self {
            id: prof.id.to_string(),
            login: prof.login,
            name: prof.name.unwrap_or_default(),
        }
    }
}

impl From<&PullRequest> for Row<'_> {
    fn from(pr: &PullRequest) -> Self {
        let pr = pr.clone();
        Row::new(vec![pr.id, pr.title, pr.repo])
    }
}
