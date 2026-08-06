use std::{env, pin::Pin, sync::Arc};

use anyhow::Context as _;
use poise::serenity_prelude::{self as serenity};
use tokio::net::TcpListener;
use tracing::{Level, error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    if let Err(e) = try_main().await {
        eprintln!("{e:?}");
        std::process::exit(1);
    }
}

fn on_error(
    error: poise::FrameworkError<'_, gale_bot::State, gale_bot::Error>,
) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(async move {
        match error {
            poise::FrameworkError::Command { error, ctx, .. } => {
                error!(err = ?error, "error while executing command");

                if let Err(err) = ctx
                    .say(format!(
                        "An error occurred while executing the command: {error:#}"
                    ))
                    .await
                {
                    error!(?err, "error while sending error message");
                }
            }
            err => {
                if let Err(err) = poise::builtins::on_error(err).await {
                    error!(?err, "error while handling error");
                }
            }
        }
    })
}

async fn try_main() -> anyhow::Result<()> {
    let reqwest_client = reqwest::Client::builder()
        .user_agent(format!("gale-bot/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to build reqwest client")?;

    let state = gale_bot::State {
        http: reqwest_client.clone(),
    };

    let token = env::var("DISCORD_TOKEN").context("DISCORD_TOKEN not set")?;

    let http = serenity::HttpBuilder::new(token)
        .client(reqwest_client)
        .build();

    let state_2 = state.clone();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: gale_bot::commands(),
            on_error,
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(state_2)
            })
        })
        .build();

    let intents =
        serenity::GatewayIntents::non_privileged().union(serenity::GatewayIntents::MESSAGE_CONTENT);
    let mut client = serenity::ClientBuilder::new_with_http(http, intents)
        .framework(framework)
        .await
        .context("failed to build serenity client")?;

    start_release_webhook_listener(state, Arc::clone(&client.http))
        .await
        .context("failed to start release webhook listener")?;

    client.start().await.context("error running bot")?;
    Ok(())
}

async fn start_release_webhook_listener(
    state: gale_bot::State,
    http: Arc<serenity::Http>,
) -> anyhow::Result<()> {
    let port = env::var("RELEASE_WEBHOOK_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .context("failed to parse RELEASE_WEBHOOK_PORT")?;

    let secret = env::var("RELEASE_WEBHOOK_SECRET")
        .map(gale_bot::releases::GithubWebhookSecret::new)
        .context("RELEASE_WEBHOOK_SECRET not set")?;

    info!(port, "listening for release webhooks");

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    tokio::spawn(async {
        match gale_bot::releases::listen(state, http, listener, secret).await {
            Ok(_) => {}
            Err(err) => {
                error!(?err, "failed to run release webhook listener");
            }
        }
    });

    Ok(())
}
