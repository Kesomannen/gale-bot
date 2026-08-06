use crate::prelude::*;

mod sync_profile;

pub fn list() -> Vec<poise::Command<State, Error>> {
    vec![ping(), sync_profile::syncprofile()]
}

/// Check if the bot is alive.
#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result {
    ctx.reply("Pong!").await?;
    Ok(())
}
