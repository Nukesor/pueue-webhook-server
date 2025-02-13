use actix_web::{error::Error, http::Method, web, HttpRequest, HttpResponse};
use pueue_lib::Request;

use crate::{
    internal_prelude::*,
    pueue::get_pueue_client,
    web::{authentication::verify_authentication_header, helper::*, AppState, Payload},
};

// Index route for getting current state of the server
//pub async fn index(
//    data: web::Data<AppState>,
//    request: web::HttpRequest,
//) -> Result<HttpResponse, HttpResponse> {
//    let headers = get_headers_hash_map(request.headers())?;
//
//    // Check the credentials and signature headers of the request
//    verify_authentication_header(&data.settings, &headers, &Vec::new())?;
//
//    let socket = get_pueue_socket(&data.settings);
//
//    Ok(HttpResponse::Ok()
//        .header(http::header::CONTENT_TYPE, "application/json")
//        .body(json))
//}

/// Index route
pub async fn webhook(
    data: web::Data<AppState>,
    path_info: web::Path<String>,
    request: HttpRequest,
    body: web::Bytes,
) -> Result<HttpResponse, Error> {
    let body: Vec<u8> = body.to_vec();
    let payload = match *request.method() {
        Method::POST => get_payload(&body)?,
        _ => Payload::default(),
    };

    let headers = get_headers_hash_map(request.headers())?;
    let webhook_name = path_info.into_inner();

    // Check the credentials and signature headers of the request
    verify_authentication_header(&data.settings, &headers, &body)?;

    info!("Incoming webhook for \"{webhook_name}\":");
    debug!("Got payload: {payload:?}");

    // Create a new task with the checked parameters and webhook name
    let new_task = get_task_from_request(&data.settings, webhook_name, payload.parameters)?;

    let mut client = match get_pueue_client(&data.settings).await {
        Ok(client) => client,
        Err(err) => {
            return Ok(HttpResponse::InternalServerError()
                .body(format!("Pueue daemon cannot be reached: {err:?}")))
        }
    };

    if let Err(err) = client.send_request(Request::Add(new_task)).await {
        return Ok(HttpResponse::InternalServerError()
            .body(format!("Failed to send message to Pueue daemon: {err:?}")));
    };

    Ok(HttpResponse::Ok().finish())
}
