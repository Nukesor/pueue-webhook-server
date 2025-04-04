use std::collections::HashMap;

use actix_web::{
    error::{Error, ErrorBadRequest, ErrorUnauthorized},
    http::header::HeaderMap,
};
use handlebars::Handlebars;
use pueue_lib::message::AddRequest;

use crate::{internal_prelude::*, settings::Settings, web::Payload};

/// We do our own json handling, since Actix doesn't allow multiple extractors at once
pub fn get_payload(body: &[u8]) -> Result<Payload, Error> {
    match serde_json::from_slice(body) {
        Ok(payload) => Ok(payload),
        Err(error) => {
            let message = format!("Json error: {error}");
            warn!("{message}");
            Err(ErrorUnauthorized(message))
        }
    }
}

/// Take the HeaderMap and convert them into normal hashmap
pub fn get_headers_hash_map(map: &HeaderMap) -> Result<HashMap<String, String>, Error> {
    let mut headers = HashMap::new();

    for (key, header_value) in map.iter() {
        let key = key.as_str().to_string();
        let value = match header_value.to_str() {
            Ok(header_value) => header_value.to_string(),
            Err(error) => {
                let message = format!("Couldn't parse header: {error}");
                warn!("{message}");
                return Err(ErrorUnauthorized(message));
            }
        };

        headers.insert(key, value);
    }

    Ok(headers)
}

/// Verify that the template renders with the given parameters
pub fn verify_template_parameters(
    template: String,
    parameters: &HashMap<String, String>,
) -> Result<String, Error> {
    if !parameters.is_empty() {
        info!("Got parameters: {parameters:?}");
    }
    // Create a new handlebar instance and enable strict mode to prevent missing or malformed
    // arguments
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);

    // Check the template for render errors with the current parameter
    let result = handlebars.render_template(&template, parameters);
    match result {
        Err(error) => {
            warn!("Error rendering command with params: {parameters:?}. Error: {error:?}");
            Err(ErrorBadRequest(format!("{error:?}")))
        }
        Ok(result) => {
            if !parameters.is_empty() {
                info!("Template renders properly: {result}");
            }
            Ok(result)
        }
    }
}

/// Get a new task from a ingoing request
pub fn get_task_from_request(
    settings: &Settings,
    name: String,
    parameters: Option<HashMap<String, String>>,
) -> Result<AddRequest, Error> {
    let parameters = parameters.unwrap_or_default();

    let webhook = settings.get_webhook_by_name(&name)?;
    let command = verify_template_parameters(webhook.command, &parameters)?;

    Ok(AddRequest {
        command,
        path: webhook.cwd,
        envs: std::env::vars().collect(),
        group: "webhook".to_string(),
        enqueue_at: None,
        dependencies: Vec::new(),
        label: None,
        priority: None,
        start_immediately: false,
        stashed: false,
    })
}
