use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::path::PathBuf;

const CONFIG_DIR_NAME: &str = "mcp-server-google-adwords";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub developer_token: String,
    pub customer_id: String,
    #[serde(default)]
    pub login_customer_id: Option<String>,
}

/// Return `Option<&str>`, mapping empty strings to `None`.
fn non_empty(opt: &Option<String>) -> Option<&str> {
    opt.as_deref().filter(|s| !s.is_empty())
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/root/.config"))
        .join(CONFIG_DIR_NAME)
        .join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    let content = std::fs::read_to_string(&path).map_err(|e| {
        anyhow!(
            "Failed to read config at {}: {}.\n\
             Create it with developer_token and customer_id.\n\
             OAuth is handled via Application Default Credentials — run:\n  \
             gcloud auth application-default login --scopes=https://www.googleapis.com/auth/adwords",
            path.display(),
            e
        )
    })?;

    let cfg: Config = toml::from_str(&content)
        .map_err(|e| anyhow!("Failed to parse {}: {}", path.display(), e))?;

    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &Config) -> Result<()> {
    if cfg.developer_token.is_empty() {
        return Err(anyhow!("developer_token must be non-empty"));
    }
    if cfg.customer_id.is_empty() || !cfg.customer_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!(
            "customer_id must be a non-empty numeric string (no hyphens), got: '{}'",
            cfg.customer_id
        ));
    }

    if let Some(login_cid) = cfg.login_customer_id() {
        if !login_cid.chars().all(|c| c.is_ascii_digit()) {
            return Err(anyhow!(
                "login_customer_id must be numeric-only if set, got: '{}'",
                login_cid
            ));
        }
    }

    Ok(())
}

impl Config {
    pub fn login_customer_id(&self) -> Option<&str> {
        non_empty(&self.login_customer_id)
    }
}
