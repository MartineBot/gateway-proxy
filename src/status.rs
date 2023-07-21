use std::sync::Arc;

use crate::CONFIG;
use twilight_http::Client;
use twilight_util::{builder::embed::EmbedBuilder, link::webhook as webhook_link};

pub fn discord_log(
    client: Arc<Client>,
    color: usize,
    title: impl Into<String>,
    message: impl Into<String>,
) {
    let title = title.into();
    let message = message.into();

    tokio::spawn(async move {
        let webhook_url = CONFIG.webhook_url.clone().unwrap();
        if webhook_url.is_empty() {
            return;
        }
        let Ok((webhook_id, webhook_token)) = webhook_link::parse(&webhook_url) else {
            return;
        };
        let em = EmbedBuilder::new()
            .color(Some(color as u32).unwrap())
            .title(title)
            .description(message)
            .build();

        let _ = client
            .execute_webhook(webhook_id, &webhook_token.unwrap())
            .username("Gateway Proxy")
            .embeds(&[em]);
    });
}
