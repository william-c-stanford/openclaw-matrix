use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OpenClawConfig {
    gateway_url: Option<String>,
    token: Option<String>,
}

/// Resolve gateway config from: CLI args > env > ~/.openclaw/openclaw.json
#[allow(dead_code)]
pub struct GatewayConfig {
    pub url: String,
    pub token: Option<String>,
}

impl GatewayConfig {
    pub fn resolve(cli_url: Option<&str>, cli_token: Option<&str>) -> Option<Self> {
        // CLI args take priority (already includes env via clap)
        let file_config = load_config_file();

        let url = cli_url
            .map(String::from)
            .or_else(|| file_config.as_ref().and_then(|c| c.gateway_url.clone()));

        let url = url?; // No URL = no gateway

        let token = cli_token
            .map(String::from)
            .or_else(|| file_config.as_ref().and_then(|c| c.token.clone()));

        Some(GatewayConfig { url, token })
    }
}

fn load_config_file() -> Option<OpenClawConfig> {
    let home = dirs::home_dir()?;
    let config_path = home.join(".openclaw").join("openclaw.json");

    if !config_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&config_path).ok()?;
    serde_json::from_str(&content).ok()
}
