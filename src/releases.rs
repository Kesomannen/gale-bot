use std::sync::Arc;

use axum::{
    Extension, Router,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use hmac::{Hmac, KeyInit, Mac};
use octocrab::models::webhook_events::payload::ReleaseWebhookEventPayload;
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use sha2::Sha256;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use crate::{State, secret::Secret};

mod handle;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub channel_id: serenity::ChannelId,
    pub ping_role_id: serenity::RoleId,
    pub changelog_url: String,
    #[serde(default)]
    pub webhook_secret: Option<GithubWebhookSecret>,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct GithubWebhookSecret(Secret<String>);

impl GithubWebhookSecret {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(Secret(secret.into()))
    }
}

pub async fn listen(
    state: State,
    http: Arc<serenity::Http>,
    listener: TcpListener,
) -> crate::Result {
    if state.config.releases.webhook_secret.is_none() {
        warn!("webhook secret not set, skipping signature verification");
    }

    let router = Router::new()
        .route("/", post(index))
        .layer(Extension(state))
        .layer(Extension(http))
        .layer(TraceLayer::new_for_http());

    axum::serve(listener, router).await?;

    Ok(())
}

async fn index(
    Extension(state): Extension<State>,
    Extension(http): Extension<Arc<serenity::Http>>,
    headers: HeaderMap,
    body: String,
) -> Result<(), StatusCode> {
    if let Some(secret) = &state.config.releases.webhook_secret {
        let signature = headers
            .get("X-Hub-Signature-256")
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_str()
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        if !verify_signature(secret, signature, body.as_bytes()) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let event_type = headers
        .get("X-GitHub-Event")
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if event_type == "ping" {
        info!("received ping event from webhook");
        return Ok(());
    }

    let payload: ReleaseWebhookEventPayload =
        serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let release: handle::GithubRelease =
        serde_json::from_value(payload.release).map_err(|err| {
            warn!(?err, "failed to parse github released payload");
            StatusCode::BAD_REQUEST
        })?;

    match handle::handle_webhook(
        &state.http,
        &http,
        &state.config.releases,
        payload.action,
        release,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(err) => {
            error!(?err, "failed to handle release webhook");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn verify_signature(secret: &GithubWebhookSecret, signature: &str, body: &[u8]) -> bool {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.0.0.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(body);
    let hex = hex::encode(mac.finalize().into_bytes());

    let expected_signature = format!("sha256={hex}");
    expected_signature == signature
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature() {
        assert!(verify_signature(
            &GithubWebhookSecret::new("It's a Secret to Everybody"),
            "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17",
            b"Hello, World!"
        ));

        assert!(!verify_signature(
            &GithubWebhookSecret::new("It's a Secret to Everybody"),
            "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17",
            b"Hello, World"
        ));
    }
}
