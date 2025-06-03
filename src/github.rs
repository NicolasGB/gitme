use std::sync::Arc;

use crate::types::Repository;
use color_eyre::{
    Result,
    eyre::{Context, OptionExt, bail},
};
use graphql_client::{GraphQLQuery, Response};
use once_cell::sync::Lazy;
use repository_view::ResponseData;
use reqwest::Client;

// Reexport for From impls
pub use repository_view::*;

/// Type alias for a string representing a DateTime.
type DateTime = String;
/// Type alias for a string representing a URI.
#[allow(clippy::upper_case_acronyms)]
type URI = String;

/// Derives `GraphQLQuery` for `RepositoryView`.
///
/// - `schema_path`: Path to the GraphQL schema file.
/// - `query_path`: Path to the GraphQL query file.
/// - `response_derives`: Derives `Debug` for the response type.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/repositories.graphql",
    response_derives = "Debug"
)]
struct RepositoryView;

/// Static variable for the GitHub API GraphQL endpoint URL.
static GITHUB_URL: &str = "https://api.github.com/graphql";

/// Static variable for a thread-safe, lazily initialized `reqwest::Client`.
/// This uses `arc_swap::ArcSwap` to allow the client to be replaced after initialization.
static STATIC_INSTANCE: Lazy<arc_swap::ArcSwap<GithubClient>> =
    Lazy::new(|| arc_swap::ArcSwap::from_pointee(Default::default()));

#[derive(Default, Debug)]
pub struct GithubClient(Client);

/// Initializes the global reqwest client with a GitHub personal access token.
///
/// This function configures the client with the necessary authorization headers
/// to interact with the GitHub GraphQL API.
///
/// # Arguments
///
/// * `token`: A string slice that holds the GitHub personal access token.
pub fn initialise(token: &str) -> Result<()> {
    let client = Client::builder()
        .user_agent("gitme")
        .default_headers(
            std::iter::once((
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
            ))
            .collect(),
        )
        .build()
        .wrap_err("Could not init client with bearer token")?;

    STATIC_INSTANCE.swap(Arc::from(GithubClient(client)));
    Ok(())
}

/// Returns a clone of the current `GithubClient` instance.
///
/// This function provides access to the globally shared `GithubClient`.
pub fn instance() -> Arc<GithubClient> {
    STATIC_INSTANCE.load().clone()
}

impl GithubClient {
    /// Fetches a repository and its pull requests from the GitHub API.
    ///
    /// # Arguments
    ///
    /// * `owner`: The owner of the repository.
    /// * `name`: The name of the repository.
    ///
    /// # Returns
    ///
    /// A `Result` containing an `Option<RepositoryViewRepository>`.
    /// - `Ok(Some(repository))` if the repository is found.
    /// - `Ok(None)` if the repository is not found.
    /// - `Err(error)` if there was an issue fetching the data.
    pub async fn repository_and_pulls(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Repository>> {
        let variables = repository_view::Variables {
            owner: owner.to_string(),
            name: name.to_string(),
        };
        let body = RepositoryView::build_query(variables);

        let response = self
            .0
            .post(GITHUB_URL)
            .json(&body)
            .send()
            .await
            .wrap_err("Error requesting github api")?;

        // Check if we have an HTTP error status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            // Try to parse as GitHub API error format
            if let Ok(github_error) = serde_json::from_str::<serde_json::Value>(&error_text) {
                if let Some(message) = github_error.get("message").and_then(|m| m.as_str()) {
                    bail!("[{}/{}] ({}): {}", owner, name, status, message);
                }
            }

            bail!("[{}/{}] HTTP error {}: {}", owner, name, status, error_text);
        }

        let reqwest_response: Response<ResponseData> = response
            .json()
            .await
            .wrap_err("Could not unmarshal api response")?;

        // Handle GraphQL errors (existing logic)
        if let Some(errors) = reqwest_response.errors {
            let errors = errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<String>>()
                .join("\n");
            bail!("{errors}")
        }

        let response_data = reqwest_response.data.ok_or_eyre("Missing response data")?;

        Ok(response_data.repository.map(Repository::from))
    }
}
