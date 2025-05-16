use std::collections::BTreeMap;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Span,
    widgets::{Block, Row, StatefulWidget, Table, TableState},
};

use super::PullRequest;

#[derive(Debug, Default)]
pub struct PullRequestsListState {
    pub grouped_prs: BTreeMap<String, Vec<PullRequest>>,
    pub filtered_prs: BTreeMap<String, Vec<PullRequest>>,
    pub table_state: TableState,
    filter_query: Option<String>,
}

impl PullRequestsListState {
    pub fn scroll_down(&mut self) {
        // Calculate total number of visible rows
        let total_rows = self
            .filtered_prs
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
            .filtered_prs
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

    /// Sets the table state to the next available repository
    pub fn next_repository(&mut self) {
        if let Some(current_selected_index) = self.table_state.selected() {
            let repo_indexes = self.repository_indexes();

            if let Some(next_repo) = repo_indexes.iter().find(|i| **i > current_selected_index) {
                self.table_state.select(Some(*next_repo));
            }
        }
    }

    /// Sets the table state to the previous available repository
    pub fn previous_repository(&mut self) {
        if let Some(current_selected_index) = self.table_state.selected() {
            let repo_indexes = self.repository_indexes();

            let previous_repo = repo_indexes
                .iter()
                .take_while(|index| **index < current_selected_index)
                .copied()
                .last()
                .unwrap_or(0);

            self.table_state.select(Some(previous_repo));
        }
    }

    /// Returns all the indexes from the visible repositories
    fn repository_indexes(&self) -> Vec<usize> {
        let mut indexes = vec![];
        let mut counter = 0;
        self.filtered_prs.iter().for_each(|(_, prs)| {
            indexes.push(counter);
            // Increment the index counter (1 is the row header that is virtual)
            counter += 1 + prs.len()
        });

        indexes
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

    pub fn set_filter_query(&mut self, query: Option<String>) {
        self.filter_query = query;
        self.update_view()
    }

    pub fn clear_filter_query(&mut self) {
        self.filter_query = None;
        self.update_view();
    }

    pub fn update_view(&mut self) {
        let mut filtered_prs = BTreeMap::new();
        // Check for an active filter and it's not ""
        if let Some(query) = self.filter_query.as_ref().filter(|q| !q.is_empty()) {
            for (repo, prs) in self.grouped_prs.iter() {
                // If the query matches the repo name add all prs
                if repo.to_lowercase().contains(&query.to_lowercase()) {
                    filtered_prs.insert(repo.clone(), prs.clone());
                } else {
                    let matches: Vec<PullRequest> = prs
                        .iter()
                        .filter(|pr| {
                            // Search in the line with the same format of the display
                            let line_text =
                                format!("#{} - {}", pr.id.to_lowercase(), pr.title.to_lowercase());
                            line_text.contains(&query.to_lowercase())
                        })
                        .cloned()
                        .collect();
                    if !matches.is_empty() {
                        filtered_prs.insert(repo.clone(), matches);
                    }
                }
            }
        } else {
            filtered_prs = self.grouped_prs.clone();
        }

        // Assign the filtered prs
        self.filtered_prs = filtered_prs;

        // Handle selected state
        let total_prs = self
            .filtered_prs
            .values()
            // +1 indicates the title that is virtual
            .map(|prs| prs.len() + 1)
            .sum::<usize>();
        if let Some(selected) = self.table_state.selected() {
            if selected >= total_prs {
                self.table_state.select(Some(0));
            }
        } else if total_prs != 0 {
            // If nothing is selected it means at some point there was no rows displayed
            // To avoid issues since now we know there is something displayed, we put it
            // in the first index
            self.table_state.select(Some(0));
        }
    }

    pub fn render_table(&mut self, block: Block, area: Rect, buf: &mut Buffer) {
        let mut rows = Vec::new();
        for (group, prs) in self.filtered_prs.iter() {
            // Set repo title with a color
            let repo = Span::styled(
                format!("▼ {} ({})", group, prs.len()),
                Style::default().fg(Color::Yellow),
            );
            rows.push(Row::new([repo]));
            let prs_len = prs.len();
            prs.iter().enumerate().for_each(|(i, pr)| {
                let mut prefix = "├─";
                if i == prs_len - 1 {
                    prefix = "└─";
                }
                rows.push(Row::new([format!(
                    "  {} #{} - {}{}",
                    prefix,
                    pr.id,
                    if pr.is_draft { "✏️ " } else { "" },
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
