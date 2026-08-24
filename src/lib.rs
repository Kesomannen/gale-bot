use std::sync::Arc;

use serde::Deserialize;

mod commands;
pub mod releases;
mod secret;

pub use commands::list as commands;

mod prelude {
    pub use super::{Context, Error, Result, State};
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub releases: releases::Config,
}

#[derive(Debug, Clone)]
pub struct State {
    pub http: reqwest::Client,
    pub config: Arc<Config>,
}

pub type Error = anyhow::Error;
pub type Result<T = (), E = Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, State, Error>;

mod constants {
    use poise::serenity_prelude::Color;

    pub const GALE_BLUE: Color = Color::from_rgb(55, 151, 218);
}
