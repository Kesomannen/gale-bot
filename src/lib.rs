#[derive(Debug, Clone)]
pub struct State {
    pub http: reqwest::Client,
}
pub type Error = anyhow::Error;
pub type Result<T = (), E = Error> = std::result::Result<T, E>;
pub type Context<'a> = poise::Context<'a, State, Error>;

mod commands;
pub mod releases;

mod prelude {
    pub use super::{Context, Error, Result, State};
}

pub use commands::list as commands;
use poise::serenity_prelude::Color;

const GALE_BLUE: Color = Color::from_rgb(55, 151, 218);
