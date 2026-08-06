use std::sync::Arc;

use axum::{
    Extension, Router,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use hmac::{Hmac, KeyInit, Mac};
use octocrab::models::webhook_events::payload::ReleaseWebhookEventPayload;
use poise::serenity_prelude as serenity;
use sha2::Sha256;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{error, warn};

use crate::State;

mod handle;

#[derive(Debug, Clone)]
pub struct GithubWebhookSecret(Arc<str>);

impl GithubWebhookSecret {
    pub fn new(secret: impl Into<Arc<str>>) -> Self {
        Self(secret.into())
    }
}

pub async fn listen(
    state: State,
    http: Arc<serenity::Http>,
    listener: TcpListener,
    secret: GithubWebhookSecret,
) -> crate::Result {
    let router = Router::new()
        .route("/", post(index))
        .layer(Extension(state))
        .layer(Extension(http))
        .layer(Extension(secret))
        .layer(TraceLayer::new_for_http());

    axum::serve(listener, router).await?;

    Ok(())
}

async fn index(
    Extension(state): Extension<State>,
    Extension(http): Extension<Arc<serenity::Http>>,
    Extension(secret): Extension<GithubWebhookSecret>,
    headers: HeaderMap,
    body: String,
) -> Result<(), StatusCode> {
    let signature = headers
        .get("X-Hub-Signature-256")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    if !verify_signature(&secret, signature, body.as_bytes()) {
        return Err(StatusCode::FORBIDDEN);
    }

    let payload: ReleaseWebhookEventPayload =
        serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    let release: handle::GithubRelease =
        serde_json::from_value(payload.release).map_err(|err| {
            warn!(?err, "failed to parse github released payload");
            StatusCode::BAD_REQUEST
        })?;

    match handle::handle_webhook(&state, &http, payload.action, release).await {
        Ok(_) => Ok(()),
        Err(err) => {
            error!(?err, "failed to handle release webhook");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn verify_signature(secret: &GithubWebhookSecret, signature: &str, body: &[u8]) -> bool {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.0.as_bytes()).expect("HMAC can take key of any size");
    mac.update(body);

    let expected_signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
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
