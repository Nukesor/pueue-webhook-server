use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use actix_web::{App, HttpServer, web};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
};
use rustls_pemfile::{pkcs8_private_keys, rsa_private_keys};
use serde::Deserialize;

mod authentication;
mod helper;
mod routes;

use routes::*;

use crate::{internal_prelude::*, settings::Settings};

/// State of the actix-web application
pub struct AppState {
    settings: Settings,
}

#[derive(Deserialize, Debug, Default)]
pub struct Payload {
    parameters: Option<HashMap<String, String>>,
}

/// Initialize the web server
/// Move the address of the queue actor inside the AppState for further dispatch
/// of tasks to the actor
pub async fn run_web_server(settings: Settings) -> Result<()> {
    let settings_for_app = settings.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                settings: settings_for_app.clone(),
            }))
            .service(web::resource("/{webhook_name}").to(webhook))
        //.service(web::resource("/").to(index))
    })
    .workers(2);

    let address = format!("{}:{}", settings.domain, settings.port);

    // Load the ssl key, if something is specified in the settings
    if settings.ssl_cert_chain.is_some() && settings.ssl_private_key.is_some() {
        let chain_path = settings
            .ssl_cert_chain
            .as_ref()
            .ok_or_else(|| eyre!("Can't find ssl_cert_chain in config"))?;
        let key_path = settings
            .ssl_private_key
            .as_ref()
            .ok_or_else(|| eyre!("Can't find ssl_private_key in config"))?;

        let certs = load_certs(PathBuf::from(chain_path))?;
        let key = load_key(PathBuf::from(key_path))?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Failed to build server TLS config.".to_string())?;

        server.bind_rustls_0_23(address, config)?.run().await?;
    } else {
        server.bind(address)?.run().await?;
    }

    Ok(())
}

/// Load the passed certificates file
fn load_certs<'a>(path: PathBuf) -> Result<Vec<CertificateDer<'a>>> {
    let file = File::open(&path).context(format!("Cannot open cert at {path:?}"))?;
    let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|err| eyre!("Failed to parse daemon certificate.: {err:?}"))?
        .into_iter()
        .collect();

    Ok(certs)
}

/// Load the passed keys file.
/// Only the first key will be used. It should match the certificate.
fn load_key<'a>(path: PathBuf) -> Result<PrivateKeyDer<'a>> {
    let file = File::open(&path).context(format!("Cannot open key {path:?}"))?;

    // Try to read pkcs8 format first
    let keys = pkcs8_private_keys(&mut BufReader::new(&file))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|_| eyre!("Failed to parse pkcs8 format."));

    if let Ok(keys) = keys
        && let Some(key) = keys.into_iter().next()
    {
        return Ok(PrivateKeyDer::Pkcs8(key));
    }

    // Try the normal rsa format afterwards.
    let keys = rsa_private_keys(&mut BufReader::new(file))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|_| eyre!("Failed to parse daemon key."))?;

    if let Some(key) = keys.into_iter().next() {
        return Ok(PrivateKeyDer::Pkcs1(key));
    }

    bail!("Can't extract private key from keyfile {path:?}")
}
