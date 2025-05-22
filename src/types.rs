#![allow(unused)]

use crate::github::{
    self, RepositoryViewRepository, RepositoryViewRepositoryPullRequestsEdgesNode,
    RepositoryViewRepositoryPullRequestsEdgesNodeAssigneesEdgesNode,
    RepositoryViewRepositoryPullRequestsEdgesNodeAuthor,
    RepositoryViewRepositoryPullRequestsEdgesNodeBaseRepository,
    RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNode,
    RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNodeAuthor,
    RepositoryViewRepositoryPullRequestsEdgesNodeReviewRequestsEdgesNodeRequestedReviewer,
    RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNode,
    RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeAuthor,
    RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNode,
    RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNodeAuthor,
};

#[derive(Debug, Clone, Default)]
pub struct Repository {
    pub name: String,
    pub url: String,
    pub pull_requests: Vec<PullRequest>,
}

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub base_repo: Repository,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub number: u64,
    pub title: String,
    pub author: User,
    pub body: String,
    pub url: String,
    pub is_draft: bool,
    pub review_requests: Vec<User>,
    pub reviews: Vec<Review>,
    pub comments: Vec<Comment>,
    pub assignees: Vec<User>,
}

#[derive(Debug, Clone, Default)]
pub struct User {
    pub name: String,
    pub login: String,
}

#[derive(Debug, Clone)]
pub struct Review {
    pub author: User,
    pub body: String,
    pub state: String,
    pub submitted_at: String,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub author: User,
    pub body: String,
}

// REGION: Repository Requests

// Converts a GitHub RepositoryViewRepository (API response) into our internal Repository struct.

impl From<RepositoryViewRepository> for Repository {
    fn from(repo: RepositoryViewRepository) -> Self {
        Repository {
            name: repo.name,
            url: repo.url,
            pull_requests: repo
                .pull_requests
                .edges
                .unwrap_or_default()
                .into_iter()
                .filter_map(|edge| edge.and_then(|e| e.node.map(PullRequest::from)))
                .collect(),
        }
    }
}

//  Converts a Base repository from a Pull request node into our internal Repository struct.
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeBaseRepository> for Repository {
    fn from(value: RepositoryViewRepositoryPullRequestsEdgesNodeBaseRepository) -> Self {
        Repository {
            name: value.name,
            url: value.url,
            pull_requests: Vec::new(),
        }
    }
}

// REGION: Pull Requests

// Converts a GitHub PullRequest node into our internal PullRequest struct.
impl From<RepositoryViewRepositoryPullRequestsEdgesNode> for PullRequest {
    fn from(pr: RepositoryViewRepositoryPullRequestsEdgesNode) -> Self {
        PullRequest {
            base_repo: pr
                .base_repository
                .map_or_else(Repository::default, Repository::from),
            base_ref_name: pr.base_ref_name,
            head_ref_name: pr.head_ref_name,
            url: pr.url,
            title: pr.title,
            body: pr.body,
            number: pr.number as u64,
            author: pr.author.map(User::from).unwrap_or_default(),
            is_draft: pr.is_draft,
            review_requests: pr
                .review_requests
                .and_then(|rr| rr.edges)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|edge| {
                    edge.and_then(|e| e.node.and_then(|n| n.requested_reviewer.map(User::from)))
                })
                .collect(),
            reviews: pr
                .reviews
                .and_then(|r| r.edges)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| e.and_then(|review| review.node.map(Review::from)))
                .collect(),
            comments: pr
                .comments
                .edges
                .unwrap_or_default()
                .into_iter()
                .filter_map(|edge| edge.and_then(|e| e.node.map(Comment::from)))
                .collect(),
            assignees: pr
                .assignees
                .edges
                .unwrap_or_default()
                .into_iter()
                .filter_map(|edge| edge.and_then(|e| e.node.map(User::from)))
                .collect(),
        }
    }
}

// Converts a PullRequest author node into a User (for PR author field).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeAuthor> for User {
    fn from(author: RepositoryViewRepositoryPullRequestsEdgesNodeAuthor) -> Self {
        let name = match author.on {
            github::RepositoryViewRepositoryPullRequestsEdgesNodeAuthorOn::User(on_user) => {
                on_user.name.unwrap_or_default()
            }
            _ => "".to_string(),
        };
        User {
            name,
            login: author.login,
        }
    }
}

// Converts a review request user node into a User (for requested reviewers in PRs).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeReviewRequestsEdgesNodeRequestedReviewer>
    for User
{
    fn from(
        value: RepositoryViewRepositoryPullRequestsEdgesNodeReviewRequestsEdgesNodeRequestedReviewer,
    ) -> Self {
        match value {
            RepositoryViewRepositoryPullRequestsEdgesNodeReviewRequestsEdgesNodeRequestedReviewer::User(user) => {
                User{ name: user.name.unwrap_or_default(), login: user.login }
            },
            _ => User::default(),
        }
    }
}

// REGION: Reviews

// Converts a GitHub Review node into our internal Review struct.
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNode> for Review {
    fn from(review: RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNode) -> Self {
        let state = match &review.state {
            github::PullRequestReviewState::APPROVED => "APPROVED",
            github::PullRequestReviewState::CHANGES_REQUESTED => "CHANGES REQUESTED",
            github::PullRequestReviewState::COMMENTED => "COMMENTED",
            github::PullRequestReviewState::DISMISSED => "DISMISSED",
            github::PullRequestReviewState::PENDING => "PENDING",
            github::PullRequestReviewState::Other(a) => a.as_str(),
        };

        Review {
            author: review.author.map_or_else(User::default, |a| a.into()),
            body: review.body,
            state: state.to_string(),
            submitted_at: review.submitted_at.unwrap_or_default(),
            comments: review
                .comments
                .edges
                .unwrap_or_default()
                .into_iter()
                .filter_map(|node| node.and_then(|c| c.node.map(Comment::from)))
                .collect(),
        }
    }
}

// Converts a review author node into a User (for Review author field).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeAuthor> for User {
    fn from(author: RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeAuthor) -> Self {
        let name = match author.on {
            github::RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeAuthorOn::User(
                u,
            ) => u.name.unwrap_or_default(),
            _ => "".to_string(),
        };
        User {
            name,
            login: author.login,
        }
    }
}

// Converts a review comment node into a Comment (for comments inside a review).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNode>
    for Comment
{
    fn from(
        comment: RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNode,
    ) -> Self {
        Comment {
            body: comment.body,

            author: comment.author.map_or_else(User::default, |a| a.into()),
        }
    }
}

// Converts a review comment author node into a User (for review comment author).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNodeAuthor>
    for User
{
    fn from(
        author: RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNodeAuthor,
    ) -> Self {
        match author {
            RepositoryViewRepositoryPullRequestsEdgesNodeReviewsEdgesNodeCommentsEdgesNodeAuthor::User(u) => {
                User{
                    name: u.name.unwrap_or_default(),
                    login: "".to_string(),
                }
            },
            _ => User::default()
        }
    }
}

impl From<RepositoryViewRepositoryPullRequestsEdgesNodeAssigneesEdgesNode> for User {
    fn from(author: RepositoryViewRepositoryPullRequestsEdgesNodeAssigneesEdgesNode) -> Self {
        User {
            name: "".to_string(),
            login: author.login,
        }
    }
}

// REGION: Comments

// Converts a PR comment author node into a User (for top-level PR comments).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNodeAuthor> for User {
    fn from(author: RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNodeAuthor) -> Self {
        let name = match author.on {
            github::RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNodeAuthorOn::User(on_user) => {
                on_user.name.unwrap_or_default()
            }
            _ => "".to_string(),
        };
        User {
            name,
            login: author.login,
        }
    }
}

// Converts a PR comment node into a Comment (for top-level PR comments).
impl From<RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNode> for Comment {
    fn from(comment: RepositoryViewRepositoryPullRequestsEdgesNodeCommentsEdgesNode) -> Self {
        Self {
            author: comment.author.map_or_else(User::default, |a| a.into()),
            body: comment.body,
        }
    }
}
