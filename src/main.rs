mod pueue;
mod settings;
mod web;

use anyhow::Result;
use log::info;

use simplelog::{Config, LevelFilter, SimpleLogger};

use crate::pueue::get_pueue_socket;
use crate::settings::Settings;
use crate::web::run_web_server;

#[actix_web::main]
async fn main() -> Result<()> {
    let _ = SimpleLogger::init(LevelFilter::Info, Config::default());
    let settings = Settings::new()?;

    info!("Check once if a Pueue daemon is available");
    get_pueue_socket(&settings).await?;

    info!("Init webserver");
    run_web_server(settings).await?;

    Ok(())
}
