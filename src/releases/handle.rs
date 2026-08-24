use std::fmt::Display;

use anyhow::Context;
use octocrab::models::webhook_events::payload::ReleaseWebhookEventAction;
use poise::serenity_prelude::{
    self as serenity, CreateEmbed, CreateMessage, EditMessage, GetMessages, MessageId,
};
use serde::Deserialize;
use tracing::{info, instrument};

use crate::prelude::*;

#[derive(Debug, PartialEq, Eq, PartialOrd)]
struct DownloadAsset {
    name: String,
    url: String,
    platform: Platform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Platform {
    Linux,
    Windows,
}

impl Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Windows => write!(f, "Windows"),
            Platform::Linux => write!(f, "Linux"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

impl DownloadAsset {
    fn from_github_asset(asset: GithubReleaseAsset) -> Option<Self> {
        let extension = asset.name.split('.').last()?;
        let platform = match extension {
            "msi" => Platform::Windows,
            "AppImage" | "rpm" | "deb" => Platform::Linux,
            _ => return None,
        };

        Some(Self {
            name: extension.to_string(),
            url: asset.browser_download_url,
            platform,
        })
    }
}

impl Ord for DownloadAsset {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.platform
            .cmp(&other.platform)
            .then_with(|| self.name.cmp(&other.name))
    }
}

#[instrument(skip(http, discord_http, config, release), fields(version = release.tag_name))]
pub async fn handle_webhook(
    http: &reqwest::Client,
    discord_http: &serenity::Http,
    config: &super::Config,
    action: ReleaseWebhookEventAction,
    release: GithubRelease,
) -> Result {
    let mut downloads = release
        .assets
        .into_iter()
        .filter_map(DownloadAsset::from_github_asset)
        .collect::<Vec<_>>();

    downloads.sort();

    let embed = create_update_embed(http, config, &release.tag_name, downloads).await?;

    match action {
        ReleaseWebhookEventAction::Created => {
            send_update_notification(discord_http, config, &release.tag_name, embed).await
        }
        ReleaseWebhookEventAction::Edited => {
            let messages = config
                .channel_id
                .messages(discord_http, GetMessages::default().limit(1))
                .await
                .context("failed to fetch messages")?;

            match messages.first() {
                Some(message) if message.content.contains(&release.tag_name) => {
                    edit_update_notification(discord_http, config, message.id, embed).await
                }
                _ => {
                    info!("no existing release notification message found, sending a new one");
                    send_update_notification(discord_http, config, &release.tag_name, embed).await
                }
            }
        }
        action => {
            info!("unhandled webhook action: {action:?}");
            Ok(())
        }
    }
}

async fn send_update_notification(
    discord_http: &serenity::Http,
    config: &super::Config,
    version: &str,
    embed: CreateEmbed,
) -> Result {
    let message = CreateMessage::default()
        .content(format!("<@&{}> ||{version}||", config.ping_role_id))
        .embed(embed);

    config
        .channel_id
        .send_message(discord_http, message)
        .await
        .context("failed to send message")?;

    Ok(())
}

async fn edit_update_notification(
    discord_http: &serenity::Http,
    config: &super::Config,
    message_id: MessageId,
    embed: CreateEmbed,
) -> Result {
    info!("editing existing release notification message");

    let edit = EditMessage::default().embed(embed);

    config
        .channel_id
        .edit_message(discord_http, message_id, edit)
        .await
        .context("failed to edit message")?;

    Ok(())
}

async fn create_update_embed(
    http: &reqwest::Client,
    config: &super::Config,
    version: &str,
    downloads: Vec<DownloadAsset>,
) -> Result<CreateEmbed> {
    let changes = get_version_changes(http, config, version).await?;

    let downloads_list = downloads
        .iter()
        .map(|download| {
            format!(
                "- [{}]({}) ({})",
                download.name, download.url, download.platform
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let description = format!("# v{version}\n\n{changes}\n### Downloads\n\n{downloads_list}");

    let embed = CreateEmbed::default()
        .title("A new update released!")
        .description(description)
        .color(crate::constants::GALE_BLUE);

    // let message = CreateMessage::default()
    //     .content(format!("<@&{}>", PING_ROLE_ID))
    //     .embed(embed);

    Ok(embed)
}

async fn get_version_changes(
    http: &reqwest::Client,
    config: &super::Config,
    version: &str,
) -> Result<String> {
    let changelog = fetch_changelog(http, &config.changelog_url).await?;
    extract_version_changes(&changelog, version)
}

fn extract_version_changes(changelog: &str, version: &str) -> Result<String> {
    let start_line = changelog
        .lines()
        .position(|line| line.contains(version))
        .context("version not found in changelog")?;
    let end_line = changelog
        .lines()
        .enumerate()
        .skip(start_line + 1)
        .find_map(|(index, line)| line.starts_with("## ").then_some(index))
        .unwrap_or_else(|| changelog.lines().count());

    let changes = changelog
        .lines()
        .skip(start_line + 1)
        .take(end_line - start_line - 1)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(changes.trim().to_string())
}

async fn fetch_changelog(http: &reqwest::Client, url: &str) -> Result<String> {
    let changelog = http
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    Ok(changelog)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CHANGELOG: &str = include_str!("../../fixtures/changelog.md");

    #[test]
    fn extract_versions() {
        for version in ["0.1.0", "0.5.0", "1.0.0", "1.12.0", "Unreleased"] {
            let changes = extract_version_changes(TEST_CHANGELOG, version).unwrap();
            insta::assert_snapshot!(format!("extract_version_{version}"), changes);
        }
    }
}
