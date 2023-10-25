use std::sync::Arc;

use crate::CONFIG;
use tracing::warn;
use twilight_http::Client;
use twilight_util::{builder::embed::EmbedBuilder, link::webhook as webhook_link};

pub fn discord_log(
    client: Arc<Client>,
    color: usize,
    title: impl Into<String>,
    message: impl Into<String>,
) {
    let title: String = title.into();
    let message: String = message.into();

    tokio::spawn(async move {
        let binding = "".to_string();
        let webhook_url = CONFIG.webhook_url.as_ref().unwrap_or(&binding);
        if webhook_url.is_empty() {
            return;
        }
        let Ok((webhook_id, webhook_token)) = webhook_link::parse(&webhook_url) else {
            warn!("Invalid webhook URL");
            return;
        };
        let em = EmbedBuilder::new()
            .color(color as u32)
            .title(title)
            .description(message)
            .build();

        client
            .execute_webhook(webhook_id, webhook_token.unwrap())
            .username("Gateway Proxy")
            .embeds(&[em])
            .await
            .expect("Failed to send webhook message");
    });
}
