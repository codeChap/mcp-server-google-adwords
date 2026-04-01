use rmcp::{
    ErrorData as McpError, ServerHandler, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
};
use serde_json::json;
use std::sync::Arc;
use tracing::debug;

use crate::api::{ApiError, GoogleAdsClient, format_response};
use crate::params::{
    CreateBudgetParams, CreateCampaignParams, GetCampaignStatsParams, ListBudgetsParams,
    ListCampaignsParams, SearchParams, UpdateBudgetParams, UpdateCampaignStatusParams,
    validate_channel_type, validate_date, validate_numeric_id, validate_status,
};

/// The MCP server wrapping the Google Ads API.
#[derive(Clone)]
pub struct GoogleAdsServer {
    client: Arc<GoogleAdsClient>,
    tool_router: ToolRouter<Self>,
}

// ---------------------------------------------------------------------------
// Result helpers — eliminates repeated match arms in every tool
// ---------------------------------------------------------------------------

fn to_tool_result(result: Result<serde_json::Value, ApiError>) -> Result<CallToolResult, McpError> {
    match result {
        Ok(resp) => Ok(CallToolResult::success(vec![Content::text(
            format_response(&resp),
        )])),
        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

#[tool_router]
impl GoogleAdsServer {
    pub fn new(client: GoogleAdsClient) -> Self {
        Self {
            client: Arc::new(client),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List Google Ads campaigns. Optionally filter by status (ENABLED/PAUSED/REMOVED) and limit results."
    )]
    async fn list_campaigns(
        &self,
        Parameters(p): Parameters<ListCampaignsParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(status = ?p.status, limit = ?p.limit, "list_campaigns called");

        let limit = p.limit.unwrap_or(100).min(1000);
        let mut query = String::from(
            "SELECT campaign.id, campaign.name, campaign.status, \
             campaign.advertising_channel_type, campaign.campaign_budget \
             FROM campaign",
        );

        if let Some(ref status) = p.status {
            let status = status.to_uppercase();
            validate_status(&status)?;
            query.push_str(&format!(" WHERE campaign.status = '{}'", status));
        }

        query.push_str(&format!(" LIMIT {}", limit));

        to_tool_result(self.client.search(&query, None).await)
    }

    #[tool(
        description = "Create a new Google Ads campaign. Requires a name, budget_id, and advertising_channel_type. \
                       Defaults to PAUSED status and MAXIMIZE_CLICKS bidding."
    )]
    async fn create_campaign(
        &self,
        Parameters(p): Parameters<CreateCampaignParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(name = %p.name, "create_campaign called");

        validate_numeric_id(&p.budget_id, "budget_id")?;

        let channel_type = p.advertising_channel_type.to_uppercase();
        validate_channel_type(&channel_type)?;

        let status = p
            .status
            .as_deref()
            .unwrap_or("PAUSED")
            .to_uppercase();
        if status != "ENABLED" && status != "PAUSED" {
            return Err(McpError::invalid_params(
                "status for new campaign must be ENABLED or PAUSED",
                None,
            ));
        }

        let bidding = p
            .bidding_strategy
            .as_deref()
            .unwrap_or("MAXIMIZE_CLICKS")
            .to_uppercase();

        if let Some(ref d) = p.start_date {
            validate_date(d, "start_date")?;
        }
        if let Some(ref d) = p.end_date {
            validate_date(d, "end_date")?;
        }

        let budget_resource = self.client.resource_name("campaignBudgets", &p.budget_id);

        let mut campaign = json!({
            "name": p.name,
            "advertisingChannelType": channel_type,
            "status": status,
            "biddingStrategyType": bidding,
            "campaignBudget": budget_resource,
            "networkSettings": {
                "targetGoogleSearch": true,
                "targetSearchNetwork": false,
                "targetContentNetwork": false
            }
        });

        if let Some(ref d) = p.start_date {
            campaign["startDate"] = json!(d.replace('-', ""));
        }
        if let Some(ref d) = p.end_date {
            campaign["endDate"] = json!(d.replace('-', ""));
        }

        let operation = json!({ "create": campaign });

        to_tool_result(self.client.mutate("campaigns", vec![operation]).await)
    }

    #[tool(
        description = "Update the status of a Google Ads campaign. Set to ENABLED, PAUSED, or REMOVED."
    )]
    async fn update_campaign_status(
        &self,
        Parameters(p): Parameters<UpdateCampaignStatusParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(campaign_id = %p.campaign_id, status = %p.status, "update_campaign_status called");

        validate_numeric_id(&p.campaign_id, "campaign_id")?;
        let status = p.status.to_uppercase();
        validate_status(&status)?;

        let resource_name = self.client.resource_name("campaigns", &p.campaign_id);

        let operation = json!({
            "update": {
                "resourceName": resource_name,
                "status": status
            },
            "updateMask": "status"
        });

        to_tool_result(self.client.mutate("campaigns", vec![operation]).await)
    }

    #[tool(
        description = "Get performance stats for a campaign over a date range. \
                       Returns impressions, clicks, cost, conversions, and CTR."
    )]
    async fn get_campaign_stats(
        &self,
        Parameters(p): Parameters<GetCampaignStatsParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(campaign_id = %p.campaign_id, "get_campaign_stats called");

        validate_numeric_id(&p.campaign_id, "campaign_id")?;
        validate_date(&p.start_date, "start_date")?;
        validate_date(&p.end_date, "end_date")?;

        let query = format!(
            "SELECT campaign.id, campaign.name, \
             metrics.impressions, metrics.clicks, metrics.cost_micros, \
             metrics.all_conversions, metrics.ctr \
             FROM campaign \
             WHERE campaign.id = {} \
               AND segments.date BETWEEN '{}' AND '{}'",
            p.campaign_id, p.start_date, p.end_date
        );

        to_tool_result(self.client.search(&query, None).await)
    }

    #[tool(description = "List Google Ads campaign budgets with their IDs, names, and amounts.")]
    async fn list_budgets(
        &self,
        Parameters(p): Parameters<ListBudgetsParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(limit = ?p.limit, "list_budgets called");

        let limit = p.limit.unwrap_or(100).min(1000);
        let query = format!(
            "SELECT campaign_budget.id, campaign_budget.name, \
             campaign_budget.amount_micros, campaign_budget.status \
             FROM campaign_budget \
             LIMIT {}",
            limit
        );

        to_tool_result(self.client.search(&query, None).await)
    }

    #[tool(
        description = "Create a new campaign budget. Amount is in micros (e.g. 10000000 = $10/day)."
    )]
    async fn create_budget(
        &self,
        Parameters(p): Parameters<CreateBudgetParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(name = %p.name, amount = p.amount_micros, "create_budget called");

        if p.amount_micros == 0 {
            return Err(McpError::invalid_params(
                "amount_micros must be greater than 0",
                None,
            ));
        }

        let operation = json!({
            "create": {
                "name": p.name,
                "amountMicros": p.amount_micros.to_string(),
                "deliveryMethod": "STANDARD"
            }
        });

        to_tool_result(self.client.mutate("campaignBudgets", vec![operation]).await)
    }

    #[tool(
        description = "Update an existing campaign budget amount. Amount is in micros (e.g. 10000000 = $10/day)."
    )]
    async fn update_budget(
        &self,
        Parameters(p): Parameters<UpdateBudgetParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(budget_id = %p.budget_id, amount = p.amount_micros, "update_budget called");

        validate_numeric_id(&p.budget_id, "budget_id")?;

        let resource_name = self.client.resource_name("campaignBudgets", &p.budget_id);

        let operation = json!({
            "update": {
                "resourceName": resource_name,
                "amountMicros": p.amount_micros.to_string()
            },
            "updateMask": "amount_micros"
        });

        to_tool_result(self.client.mutate("campaignBudgets", vec![operation]).await)
    }

    #[tool(
        description = "Execute a raw GAQL (Google Ads Query Language) query. Returns JSON rows."
    )]
    async fn search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(query = %p.query, "search called");

        if p.query.trim().is_empty() {
            return Err(McpError::invalid_params("query must not be empty", None));
        }

        to_tool_result(self.client.search(&p.query, None).await)
    }
}

// ---------------------------------------------------------------------------
// MCP ServerHandler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for GoogleAdsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "google-adwords-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Google Ads MCP server. Tools: list_campaigns, create_campaign, \
                 update_campaign_status, get_campaign_stats, list_budgets, create_budget, \
                 update_budget, search.",
            )
    }
}
