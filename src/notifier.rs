use anyhow::{Context, Result, anyhow};
use reqwest::Client;

#[derive(serde::Serialize)]
struct DiscordPayload<'a> {
    content: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<&'a str>,
}

pub struct Notifier {
    client: Client,
    webhook_url: String,
    bot_name: Option<String>,
}

impl Notifier {
    pub fn new(webhook_url: String, bot_name: Option<String>) -> Self {
        Self {
            client: Client::new(),
            webhook_url,
            bot_name,
        }
    }

    pub async fn send(&self, content: &str) -> Result<()> {
        let payload = DiscordPayload {
            content,
            username: self.bot_name.as_deref(),
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .context("failed to send webhook request")?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(anyhow!("discord webhook returned {status}: {body}"))
    }
}
