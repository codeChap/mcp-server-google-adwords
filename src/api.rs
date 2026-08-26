use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;
use tracing::instrument;

use crate::auth::GoogleAuth;
use crate::config::Config;

const API_VERSION: &str = "v23";
const USER_AGENT: &str = concat!("google-adwords-mcp/", env!("CARGO_PKG_VERSION"));

/// Errors returned by the Google Ads API client.
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("Token error: {0}")]
    Token(#[from] anyhow::Error),

    #[error("Google Ads API error ({status}): {body}")]
    Api {
        status: reqwest::StatusCode,
        body: String,
    },
}

/// Google Ads REST API client.
pub struct GoogleAdsClient {
    customer_id: String,
    developer_token: String,
    login_customer_id: Option<String>,
    auth: GoogleAuth,
    http: Client,
}

impl GoogleAdsClient {
    pub fn new(config: &Config, auth: GoogleAuth, http: Client) -> Self {
        Self {
            customer_id: config.customer_id.clone(),
            developer_token: config.developer_token.clone(),
            login_customer_id: config.login_customer_id().map(String::from),
            auth,
            http,
        }
    }

    /// Build a shared reqwest::Client with custom user agent.
    pub fn build_http_client() -> Client {
        Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client")
    }

    /// Build a full resource name: `customers/{cid}/{resource}/{id}`.
    pub fn resource_name(&self, resource: &str, id: &str) -> String {
        format!("customers/{}/{}/{}", self.customer_id, resource, id)
    }

    fn base_url(&self) -> String {
        format!(
            "https://googleads.googleapis.com/{}/customers/{}",
            API_VERSION, self.customer_id
        )
    }

    /// Execute a GAQL search query. Returns the parsed JSON response.
    #[instrument(skip(self), fields(query = %query))]
    pub async fn search(&self, query: &str, page_size: Option<u32>) -> Result<Value, ApiError> {
        let url = format!("{}/googleAds:search", self.base_url());
        let body = SearchRequest {
            query: query.to_string(),
            page_size,
        };
        self.post(&url, &body).await
    }

    /// Execute a mutate operation on a resource (e.g., campaigns, campaignBudgets).
    #[instrument(skip(self, operations), fields(resource = %resource))]
    pub async fn mutate(&self, resource: &str, operations: Vec<Value>) -> Result<Value, ApiError> {
        let url = format!("{}/{}:mutate", self.base_url(), resource);
        let body = MutateRequest {
            operations,
            partial_failure: false,
            validate_only: false,
        };
        self.post(&url, &body).await
    }

    /// Send an authenticated POST request and parse the JSON response.
    async fn post<Req: Serialize>(&self, url: &str, body: &Req) -> Result<Value, ApiError> {
        let access_token = self.auth.access_token().await?;

        let mut builder = self
            .http
            .post(url)
            .bearer_auth(&access_token)
            .header("developer-token", &self.developer_token)
            .json(body);

        if let Some(ref login_cid) = self.login_customer_id {
            builder = builder.header("login-customer-id", login_cid);
        }

        if let Some(qp) = self.auth.quota_project() {
            builder = builder.header("x-goog-user-project", qp);
        }

        let response = builder.send().await?;
        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<failed to read body: {e}>"));
            tracing::warn!(status = %status, "Google Ads API request failed");
            return Err(ApiError::Api { status, body });
        }

        Ok(response.json::<Value>().await?)
    }
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SearchRequest {
    query: String,
    #[serde(rename = "pageSize", skip_serializing_if = "Option::is_none")]
    page_size: Option<u32>,
}

#[derive(Serialize)]
struct MutateRequest {
    operations: Vec<Value>,
    #[serde(rename = "partialFailure")]
    partial_failure: bool,
    #[serde(rename = "validateOnly")]
    validate_only: bool,
}

// ---------------------------------------------------------------------------
// Response formatting
// ---------------------------------------------------------------------------

/// Pretty-print an API response. For search responses, extracts the `results` array.
pub fn format_response(resp: &Value) -> String {
    let target = resp.get("results").unwrap_or(resp);
    serde_json::to_string_pretty(target).unwrap_or_else(|_| resp.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_response_extracts_results() {
        let resp = json!({
            "results": [{"campaign": {"id": "123", "name": "Test"}}],
            "fieldMask": "campaign.id,campaign.name"
        });
        let out = format_response(&resp);
        assert!(out.contains("\"id\": \"123\""));
        assert!(!out.contains("fieldMask"));
    }

    #[test]
    fn format_response_no_results_key() {
        let resp = json!({
            "results": { "resourceName": "customers/123/campaigns/456" }
        });
        let out = format_response(&resp);
        assert!(out.contains("resourceName"));
    }

    #[test]
    fn format_response_mutate() {
        let resp = json!({
            "results": [{"resourceName": "customers/123/campaigns/456"}]
        });
        let out = format_response(&resp);
        assert!(out.contains("customers/123/campaigns/456"));
    }
}
