use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

use octocrab::{
    Page,
    params::{Direction, State},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, Row, StatefulWidget, Table, TableState, Widget},
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

    prs: PullRequestsListState,

    details: PullRequestsDetailsState,

    loading_state: LoadingState,
}

#[derive(Debug, Default)]
struct PullRequestsListState {
    grouped_prs: std::collections::BTreeMap<String, Vec<PullRequest>>,
    expanded_repos: std::collections::HashSet<String>,
    table_state: TableState,
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
}

#[derive(Debug, Clone, Copy, Default)]
enum ActivePanel {
    #[default]
    PullRequests,
    Details,
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

    // On a load of prs received, pushes them in their corresponding map entry in the prs state
    fn on_load(&self, page: &Page<OctoPullRequest>) {
        // List the pull requests filter them by the user that has to review them
        let prs: Vec<PullRequest> = page
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

        let mut state = self.state.write().unwrap();
        state.loading_state = LoadingState::Loaded;

        // Group the prs by repository to be able to better handle them later on in a tree view
        for pr in prs {
            state
                .prs
                .grouped_prs
                .entry(pr.repo.clone())
                .or_default()
                .push(pr);
        }

        // If the map is not empty, and theres not a previously selected state
        if !state.prs.grouped_prs.is_empty() && state.prs.table_state.selected().is_none() {
            state.prs.table_state.select(Some(0));
        }
    }

    fn on_err(&self, err: &octocrab::Error) {
        self.set_loading_state(LoadingState::Error(err.to_string()));
    }

    fn set_loading_state(&self, state: LoadingState) {
        self.state.write().unwrap().loading_state = state;
    }

    pub fn scroll_down(&self) {
        let mut state = self.state.write().unwrap();

        // For some reason it's overflowing and returning an index that doesn't exist when we go
        // down as the last element so we do it manually
        // Calculate total number of visible rows
        let total_rows = state.prs.grouped_prs.iter().fold(0, |acc, (repo, prs)| {
            acc + 1
                + if state.prs.expanded_repos.contains(repo) {
                    prs.len()
                } else {
                    0
                }
        });

        let current = state.prs.table_state.selected().unwrap_or(0);
        if current + 1 < total_rows {
            state.prs.table_state.scroll_down_by(1);
        }

        // If a pr is selected make it available in the details
        if let Some(index) = state.prs.table_state.selected() {
            state.details.pr_details =
                self.find_by_index(&state.prs.grouped_prs, &state.prs.expanded_repos, index);
        }
    }

    pub fn scroll_up(&self) {
        let mut state = self.state.write().unwrap();
        state.prs.table_state.scroll_up_by(1);
        // If a pr is selected make it available in the details
        if let Some(index) = state.prs.table_state.selected() {
            state.details.pr_details =
                self.find_by_index(&state.prs.grouped_prs, &state.prs.expanded_repos, index)
        }
    }

    pub fn toggle_expand(&self) {
        let repo_to_toggle = {
            let state = self.state.read().unwrap();
            // Get current row to see if it's on a group
            let index = match state.prs.table_state.selected() {
                Some(index) => index,
                // Should never be here but if nothing selected, return
                None => return,
            };

            // Now we loop through the repos to find the one that matches the index
            let mut current_index = 0;
            let mut repo_to_toggle = None;

            for (repo, prs) in state.prs.grouped_prs.iter() {
                if current_index == index {
                    repo_to_toggle = Some(repo.clone());
                    break;
                }

                // Increment for the header row of the group
                current_index += 1;

                // If the repo is expanded we need to loop through all the nested children
                if state.prs.expanded_repos.contains(repo) {
                    // To be smart we'll add the length, since we know how everything is formatted,
                    // if the length > than the current index it means the current is on a pr
                    // therefore it's not expandable
                    current_index += prs.len();
                    if current_index > index {
                        return;
                    }
                }
            }

            repo_to_toggle
        };

        let mut state = self.state.write().unwrap();
        // If there's something to toggle
        if let Some(repo) = repo_to_toggle {
            // Check if it's expanded, if so remove it
            if state.prs.expanded_repos.contains(&repo) {
                state.prs.expanded_repos.remove(&repo);
            } else {
                state.prs.expanded_repos.insert(repo);
            }
        }
    }

    pub fn open(&self) {
        let state = self.state.write().unwrap();
        if let Some(index) = state.prs.table_state.selected() {
            if let Some(pr) =
                self.find_by_index(&state.prs.grouped_prs, &state.prs.expanded_repos, index)
            {
                open::that(&pr.url).unwrap();
            }
        }
    }

    // Given an index and a btreemap find the pr that matches
    fn find_by_index(
        &self,
        grouped_prs: &std::collections::BTreeMap<String, Vec<PullRequest>>,
        expanded_repos: &HashSet<String>,
        index: usize,
    ) -> Option<PullRequest> {
        let mut current_index = 0;

        for (repo, prs) in grouped_prs.iter() {
            if current_index == index {
                // Here we're returning none, since it matches a header row
                return None;
            }
            // Increment for the header row of the group
            current_index += 1;

            // If the repo is expanded search in it otherwise skip
            if expanded_repos.contains(repo) {
                for pr in prs.iter() {
                    if index == current_index {
                        return Some(pr.clone());
                    }
                    // Increment the just seen pr
                    current_index += 1;
                }
            }
        }
        None
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

        // Create two simple boxes to visualize the splits
        let mut prs_block = Block::default()
            .title("📋 Pull Requests".bold())
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let details_box = Block::default()
            .title("Details")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        let bottom_box = Block::default()
            .title("Help")
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded);

        // Now that all the layout is ready and the blocks too, we acquire the lock
        let mut state = self.state.write().unwrap();

        //  highlight the panel with the primary terminal color
        if let ActivePanel::PullRequests = state.active_panel {
            prs_block = prs_block.border_style(Style::default().fg(Color::Green));
        }

        let loading_state = match state.loading_state {
            LoadingState::Loading => String::from("Loading...").yellow(),
            LoadingState::Error(ref e) => format!("⚠ Error: {}", e).red(),
            _ => String::from("").white(),
        };

        let help_line = Line::styled(
            format!("{loading_state} ↑↓,j/k to scroll • TAB to switch • q to quit"),
            Color::Gray,
        );

        // Draw the table
        let columns = [Constraint::Fill(1)];
        // Calculate the number of rows
        let mut rows = Vec::new();
        for (group, prs) in state.prs.grouped_prs.iter() {
            let expanded = state.prs.expanded_repos.contains(group);
            if expanded {
                // Push the group
                rows.push(Row::new([format!("▼ {} ({})", group, prs.len())]));
                // Push all it's prs
                prs.iter().for_each(|pr| {
                    rows.push(Row::new([format!("    {} - {}", pr.id, pr.title)]));
                });
            } else {
                // Push the group
                rows.push(Row::new([format!("▶ {} ({})", group, prs.len())]));
            }
        }

        let table = Table::new(rows, columns)
            .block(prs_block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::Rgb(76, 55, 67)) // #4c3743
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );

        // Render the table as a stateful widget
        StatefulWidget::render(table, prs_layout[0], buf, &mut state.prs.table_state);

        // If details inside, get the area and render it
        if let Some(pr_details) = &state.details.pr_details {
            let details_inner = details_box.inner(prs_layout[1]);
            Line::styled(&pr_details.body, Color::Gray).render(details_inner, buf);
        }
        // Render always the outer box
        details_box.render(prs_layout[1], buf);
        // Render bottom part
        // Here we get the inner of the bottom from it's layout, then we render the other two
        let bottom_inner = bottom_box.inner(base_layout[1]);
        bottom_box.render(base_layout[1], buf);
        help_line.render(bottom_inner, buf);
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
        }
    }
}

impl From<&PullRequest> for Row<'_> {
    fn from(pr: &PullRequest) -> Self {
        let pr = pr.clone();
        Row::new(vec![pr.id, pr.title, pr.repo])
    }
}
