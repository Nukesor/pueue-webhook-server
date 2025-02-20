mod pueue;
mod settings;
mod tracing;
mod web;

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
    get_pueue_client(&settings).await?;

    info!("Init webserver");
    run_web_server(settings).await?;

    Ok(())
}
