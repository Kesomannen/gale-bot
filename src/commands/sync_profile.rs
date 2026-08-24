use std::time::{Duration, Instant};

use poise::{
    CreateReply,
    serenity_prelude::{
        CreateActionRow, CreateButton, CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter,
    },
};
use serde::Deserialize;

use crate::prelude::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileMetadata {
    id: String,
    created_at: jiff::Timestamp,
    updated_at: jiff::Timestamp,
    owner: User,
    manifest: ProfileManifest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct User {
    discord_id: String,
    name: String,
    display_name: String,
    avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileManifest {
    profile_name: String,
    community: Option<String>,
    mods: Vec<ProfileMod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileMod {
    name: String,
    enabled: bool,
    version: ModVersion,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

/// Get information about a Gale sync profile.
#[poise::command(slash_command)]
pub async fn syncprofile(ctx: Context<'_>, code: String) -> Result {
    let start = Instant::now();

    let code = code.trim();

    let url = format!("https://gale.kesomannen.com/api/profile/{code}/meta");
    let response = ctx.data().http.get(&url).send().await?.error_for_status()?;

    let elapsed = start.elapsed();

    let metadata = response.json().await?;

    let embed = create_embed(metadata, elapsed);

    let buttons = CreateActionRow::Buttons(vec![
        CreateButton::new_link(format!(
            "https://gale.kesomannen.com/api/desktop/profile/sync/clone/{code}",
        ))
        .label("Import"),
    ]);

    let reply = CreateReply::default()
        .embed(embed)
        .components(vec![buttons])
        .reply(true);

    ctx.send(reply).await?;

    Ok(())
}

fn create_embed(metadata: ProfileMetadata, elapsed: Duration) -> CreateEmbed {
    let ProfileMetadata {
        id,
        created_at,
        updated_at,
        owner,
        manifest,
    } = metadata;

    CreateEmbed::default()
        .author({
            let mut embed = CreateEmbedAuthor::new(owner.display_name)
                .url(format!("https://discord.com/users/{}", owner.discord_id));
            if let Some(avatar) = owner.avatar {
                embed = embed.icon_url(format!(
                    "https://cdn.discordapp.com/avatars/{}/{}.png",
                    owner.discord_id, avatar
                ));
            }
            embed
        })
        .title(manifest.profile_name)
        .description(format!("`{id}`"))
        .fields(
            manifest
                .community
                .map(|c| ("Game", c, false))
                .into_iter()
                .chain([
                    ("Created at", format_date(&created_at), false),
                    ("Updated at", format_date(&updated_at), false),
                    ("Mod Count", manifest.mods.len().to_string(), false),
                ]),
        )
        .footer(CreateEmbedFooter::new(format!(
            "Fetched in {}ms",
            elapsed.as_millis()
        )))
        .color(crate::constants::GALE_BLUE)
}

fn format_date(timestamp: &jiff::Timestamp) -> String {
    timestamp.strftime("%Y-%m-%d %H:%M:%S").to_string()
}
