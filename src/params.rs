use rmcp::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

const VALID_CAMPAIGN_STATUSES: &[&str] = &["ENABLED", "PAUSED", "REMOVED"];
const VALID_CHANNEL_TYPES: &[&str] = &["SEARCH", "DISPLAY", "VIDEO", "PERFORMANCE_MAX"];

// ---------------------------------------------------------------------------
// Validation helpers — pure functions, no &self needed
// ---------------------------------------------------------------------------

pub fn validate_numeric_id(id: &str, field_name: &str) -> Result<(), McpError> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(McpError::invalid_params(
            format!("{field_name} must be a non-empty numeric string, got: '{id}'"),
            None,
        ));
    }
    Ok(())
}

pub fn validate_date(date: &str, field_name: &str) -> Result<(), McpError> {
    let parts: Vec<&str> = date.split('-').collect();
    let valid = parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()));
    if !valid {
        return Err(McpError::invalid_params(
            format!("{field_name} must be in YYYY-MM-DD format, got: '{date}'"),
            None,
        ));
    }
    Ok(())
}

fn validate_one_of(value: &str, allowed: &[&str], field_name: &str) -> Result<(), McpError> {
    if !allowed.contains(&value) {
        return Err(McpError::invalid_params(
            format!(
                "Invalid {} '{}'. Must be one of: {}",
                field_name,
                value,
                allowed.join(", ")
            ),
            None,
        ));
    }
    Ok(())
}

pub fn validate_status(status: &str) -> Result<(), McpError> {
    validate_one_of(status, VALID_CAMPAIGN_STATUSES, "status")
}

pub fn validate_channel_type(channel_type: &str) -> Result<(), McpError> {
    validate_one_of(
        channel_type,
        VALID_CHANNEL_TYPES,
        "advertising_channel_type",
    )
}

// ---------------------------------------------------------------------------
// Parameter structs
// ---------------------------------------------------------------------------

/// Parameters for the `list_campaigns` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCampaignsParams {
    #[schemars(description = "Filter by campaign status: ENABLED, PAUSED, or REMOVED (optional)")]
    pub status: Option<String>,

    #[schemars(description = "Maximum number of campaigns to return (default 100, max 1000)")]
    pub limit: Option<u32>,
}

/// Parameters for the `create_campaign` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCampaignParams {
    #[schemars(description = "Campaign name")]
    pub name: String,

    #[schemars(description = "Budget ID (numeric) to associate with the campaign")]
    pub budget_id: String,

    #[schemars(
        description = "Advertising channel type: SEARCH, DISPLAY, VIDEO, or PERFORMANCE_MAX"
    )]
    pub advertising_channel_type: String,

    #[schemars(
        description = "Bidding strategy type (default: MAXIMIZE_CLICKS). Options: MAXIMIZE_CLICKS, MAXIMIZE_CONVERSIONS, TARGET_CPA, TARGET_ROAS, MANUAL_CPC"
    )]
    pub bidding_strategy: Option<String>,

    #[schemars(description = "Campaign status (default: PAUSED). Options: ENABLED, PAUSED")]
    pub status: Option<String>,

    #[schemars(description = "Campaign start date in YYYY-MM-DD format (optional)")]
    pub start_date: Option<String>,

    #[schemars(description = "Campaign end date in YYYY-MM-DD format (optional)")]
    pub end_date: Option<String>,
}

/// Parameters for the `update_campaign_status` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCampaignStatusParams {
    #[schemars(description = "Campaign ID (numeric)")]
    pub campaign_id: String,

    #[schemars(description = "New status: ENABLED, PAUSED, or REMOVED")]
    pub status: String,
}

/// Parameters for the `get_campaign_stats` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCampaignStatsParams {
    #[schemars(description = "Campaign ID (numeric)")]
    pub campaign_id: String,

    #[schemars(description = "Start date in YYYY-MM-DD format")]
    pub start_date: String,

    #[schemars(description = "End date in YYYY-MM-DD format")]
    pub end_date: String,
}

/// Parameters for the `list_budgets` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBudgetsParams {
    #[schemars(description = "Maximum number of budgets to return (default 100, max 1000)")]
    pub limit: Option<u32>,
}

/// Parameters for the `create_budget` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateBudgetParams {
    #[schemars(description = "Budget name")]
    pub name: String,

    #[schemars(
        description = "Daily budget amount in micros (e.g. 10000000 = $10). Must be greater than 0."
    )]
    pub amount_micros: u64,
}

/// Parameters for the `update_budget` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateBudgetParams {
    #[schemars(description = "Budget ID (numeric)")]
    pub budget_id: String,

    #[schemars(description = "New daily budget amount in micros (e.g. 10000000 = $10)")]
    pub amount_micros: u64,
}

/// Parameters for the `search` tool (raw GAQL).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "GAQL query string")]
    pub query: String,

    #[schemars(description = "Page size for results (default 100, max 10000)")]
    pub page_size: Option<u32>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- validate_numeric_id --------------------------------------------------

    #[test]
    fn numeric_id_valid() {
        assert!(validate_numeric_id("123456", "test").is_ok());
        assert!(validate_numeric_id("0", "test").is_ok());
    }

    #[test]
    fn numeric_id_empty() {
        assert!(validate_numeric_id("", "test").is_err());
    }

    #[test]
    fn numeric_id_with_hyphens() {
        assert!(validate_numeric_id("123-456", "test").is_err());
    }

    #[test]
    fn numeric_id_with_letters() {
        assert!(validate_numeric_id("abc", "test").is_err());
    }

    // -- validate_date --------------------------------------------------------

    #[test]
    fn date_valid() {
        assert!(validate_date("2026-01-15", "test").is_ok());
        assert!(validate_date("2026-12-31", "test").is_ok());
    }

    #[test]
    fn date_wrong_format() {
        assert!(validate_date("01-15-2026", "test").is_err());
        assert!(validate_date("2026/01/15", "test").is_err());
        assert!(validate_date("20260115", "test").is_err());
        assert!(validate_date("", "test").is_err());
    }

    #[test]
    fn date_wrong_length() {
        assert!(validate_date("2026-1-5", "test").is_err());
    }

    // -- validate_status ------------------------------------------------------

    #[test]
    fn status_valid() {
        assert!(validate_status("ENABLED").is_ok());
        assert!(validate_status("PAUSED").is_ok());
        assert!(validate_status("REMOVED").is_ok());
    }

    #[test]
    fn status_invalid() {
        assert!(validate_status("ACTIVE").is_err());
        assert!(validate_status("enabled").is_err());
        assert!(validate_status("").is_err());
    }

    // -- validate_channel_type ------------------------------------------------

    #[test]
    fn channel_type_valid() {
        assert!(validate_channel_type("SEARCH").is_ok());
        assert!(validate_channel_type("DISPLAY").is_ok());
        assert!(validate_channel_type("VIDEO").is_ok());
        assert!(validate_channel_type("PERFORMANCE_MAX").is_ok());
    }

    #[test]
    fn channel_type_invalid() {
        assert!(validate_channel_type("SHOPPING").is_err());
        assert!(validate_channel_type("search").is_err());
        assert!(validate_channel_type("").is_err());
    }
}
