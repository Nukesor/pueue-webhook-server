mod pueue;
mod settings;
mod tracing;
mod web;

use std::time::Duration;

use crate::{pueue::get_pueue_client, settings::Settings, web::run_web_server};

pub(crate) mod internal_prelude {
    pub use color_eyre::{
        Result,
        eyre::{WrapErr, bail, eyre},
    };
    #[allow(unused)]
    pub(crate) use tracing::{debug, error, info, trace, warn};
}

use crate::internal_prelude::*;

#[actix_web::main]
async fn main() -> Result<()> {
    let settings = Settings::new()?;
    tracing::install_tracing(1)?;

    info!("Check once if a Pueue daemon is available");
    wait_for_pueue(&settings).await?;

    info!("Init webserver");
    run_web_server(settings).await?;

    Ok(())
}

async fn wait_for_pueue(settings: &Settings) -> Result<()> {
    // Total time limit (5 minutes)
    let max_duration = Duration::from_secs(300);
    let mut total_wait = Duration::ZERO;
    let mut backoff = Duration::from_secs(5);

    loop {
        info!("Checking if a Pueue daemon is available...");

        match get_pueue_client(settings).await {
            Ok(_) => {
                info!("Pueue daemon is available!");
                return Ok(());
            }
            Err(err) => {
                info!("Failed to connect: {err:?}");
                total_wait += backoff;

                if total_wait >= max_duration {
                    info!("Giving up after {:?}", total_wait);
                    return Err(err);
                }

                info!("Retrying in {:?}...", backoff);
                std::thread::sleep(backoff);
                backoff = std::cmp::min(backoff * 2, Duration::from_secs(30)); // cap at 30s
            }
        }
    }
}
