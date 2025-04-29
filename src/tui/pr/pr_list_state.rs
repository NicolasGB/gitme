use ratatui::widgets::TableState;

use super::PullRequest;

#[derive(Debug, Default)]
pub struct PullRequestsListState {
    //TODO: make these fields private and set them later
    pub grouped_prs: std::collections::BTreeMap<String, Vec<PullRequest>>,
    pub expanded_repos: std::collections::HashSet<String>,
    pub table_state: TableState,
}

impl PullRequestsListState {
    pub fn scroll_down(&mut self) {
        // For some reason it's overflowing and returning an index that doesn't exist when we go
        // down as the last element so we do it manually
        // Calculate total number of visible rows
        let total_rows = self.grouped_prs.iter().fold(0, |acc, (repo, prs)| {
            acc + 1
                + if self.expanded_repos.contains(repo) {
                    prs.len()
                } else {
                    0
                }
        });
        let current = self.table_state.selected().unwrap_or(0);
        if current + 1 < total_rows {
            self.table_state.scroll_down_by(1);
        }
    }

    pub fn scroll_up(&mut self) {
        self.table_state.scroll_up_by(1);
    }

    pub fn toggle_expand(&mut self) {
        let repo_to_toggle = {
            // Get current row to see if it's on a group
            let index = match self.table_state.selected() {
                Some(index) => index,
                // Should never be here but if nothing selected, return
                None => return,
            };

            // Now we loop through the repos to find the one that matches the index
            let mut current_index = 0;
            let mut repo_to_toggle = None;

            for (repo, prs) in self.grouped_prs.iter() {
                if current_index == index {
                    repo_to_toggle = Some(repo.clone());
                    break;
                }

                // Increment for the header row of the group
                current_index += 1;

                // If the repo is expanded we need to loop through all the nested children
                if self.expanded_repos.contains(repo) {
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

        // If there's something to toggle
        if let Some(repo) = repo_to_toggle {
            // Check if it's expanded, if so remove it
            if self.expanded_repos.contains(&repo) {
                self.expanded_repos.remove(&repo);
            } else {
                self.expanded_repos.insert(repo);
            }
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

        for (repo, prs) in self.grouped_prs.iter() {
            if current_index == index {
                // Here we're returning none, since it matches a header row
                return None;
            }
            // Increment for the header row of the group
            current_index += 1;

            // If the repo is expanded search in it otherwise skip
            if self.expanded_repos.contains(repo) {
                for pr in prs.iter() {
                    if index == current_index {
                        return Some(pr);
                    }
                    // Increment the just seen pr
                    current_index += 1;
                }
            }
        }
        None
    }
}
