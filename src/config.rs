use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::fs;

use crate::rules::StreamSelector;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub discord: DiscordConfig,
    pub monitor: MonitorConfig,
    pub run: Option<RunConfig>,
    pub attach: Option<AttachConfig>,
    #[serde(default)]
    pub rules: Vec<RuleConfig>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
    pub bot_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MonitorConfig {
    pub name: String,
    pub notify_on_start: Option<bool>,
    pub notify_on_finish: Option<bool>,
    pub notify_on_fail: Option<bool>,
    pub include_last_output_in_fail_message: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub pty: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AttachConfig {
    pub log_file: Option<PathBuf>,
    pub start_at_end: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RuleConfig {
    pub name: String,
    pub pattern: Option<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default = "default_stream")]
    pub stream: StreamSelector,
    pub notify_template: Option<String>,
    pub cooldown_secs: Option<u64>,
    pub max_notifications: Option<u32>,
}

fn default_stream() -> StreamSelector {
    StreamSelector::Both
}

pub async fn load_config(path: &PathBuf) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read config at {}", path.display()))?;
    let mut config: Config = toml::from_str(&contents).context("failed to parse TOML config")?;
    let config_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if let Some(run) = config.run.as_mut() {
        if let Some(cwd) = run.cwd.as_mut() {
            if cwd.is_relative() {
                *cwd = config_dir.join(&*cwd);
            }
        }
    }

    Ok(config)
}
