use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use actix_web::*;
use anyhow::{bail, Context, Result};
use config::ConfigError;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile::{pkcs8_private_keys, rsa_private_keys};
use serde::Deserialize;

mod authentication;
mod helper;
mod routes;

use crate::settings::Settings;
use routes::*;

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
            .ok_or_else(|| ConfigError::NotFound("ssl_cert_chain".to_string()))?;
        let key_path = settings
            .ssl_private_key
            .as_ref()
            .ok_or_else(|| ConfigError::NotFound("ssl_private_key".to_string()))?;

        let certs = load_certs(PathBuf::from(chain_path))?;
        let key = load_key(PathBuf::from(key_path))?;

        let config = ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_safe_default_protocol_versions()
            .expect("Couldn't enforce TLS1.2 and TLS 1.3. This is a bug.")
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("Failed to build server TLS config.".to_string())?;

        server.bind_rustls(address, config)?.run().await?;
    } else {
        server.bind(address)?.run().await?;
    }

    Ok(())
}

/// Load the passed certificates file
fn load_certs(path: PathBuf) -> Result<Vec<Certificate>> {
    let file = File::open(&path).context(format!("Cannot open cert at {path:?}"))?;
    let certs: Vec<Certificate> = rustls_pemfile::certs(&mut BufReader::new(file))
        .context("Failed to parse certificate")?
        .into_iter()
        .map(Certificate)
        .collect();

    Ok(certs)
}

/// Load the passed keys file.
/// Only the first key will be used. It should match the certificate.
fn load_key(path: PathBuf) -> Result<PrivateKey> {
    let file = File::open(&path).context(format!("Cannot open key {path:?}"))?;

    // Try to read pkcs8 format first
    let keys =
        pkcs8_private_keys(&mut BufReader::new(&file)).context("Failed to parse pkcs8 format.");

    if let Ok(keys) = keys {
        if let Some(key) = keys.into_iter().next() {
            return Ok(PrivateKey(key));
        }
    }

    // Try the normal rsa format afterwards.
    let keys =
        rsa_private_keys(&mut BufReader::new(file)).context("Failed to parse daemon key.")?;

    if let Some(key) = keys.into_iter().next() {
        return Ok(PrivateKey(key));
    }

    bail!("Couldn't extract private key from keyfile {path:?}")
}
