use ::actix::prelude::*;
use ::actix_web::*;
use ::actix_web::middleware::Logger;

use ::std::collections::HashMap;
use ::log::info;

use crate::queue_actor::QueueActor;
use crate::task::NewTask;
use crate::template::verify_template_parameters;


/// State of the actix-web application
struct AppState {
    queue_actor: Addr<QueueActor>,
}


/// Index route
fn webhook(
    data: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
    path_info: web::Path<String>,
) -> Result<HttpResponse, HttpResponse> {
    // Verify that the parameters match the required parameters in the template string
    let params = query.into_inner();
    let webhook_id = path_info.into_inner();

    info!("");
    info!("Incoming webhook for \"{}\":", webhook_id);
    verify_template_parameters("This is a test {{rofl}}".to_string(), &params)?;

    // Create a new task with the checked parameters and webhook id
    let new_task = NewTask {
        id: webhook_id,
        parameters: params,
    };

    // Send the task to the actor managing the queue
    data.queue_actor.do_send(new_task);

    Ok(HttpResponse::Ok().finish())
}

pub fn init_web_server(queue_actor: Addr<QueueActor>) {
    HttpServer::new(move || {
        App::new()
            .data(AppState {
                queue_actor: queue_actor.clone(),
            })
            .wrap(Logger::default())
            .service(web::resource("/webhook/{webhook_id}").to(webhook))
    })
    .bind("127.0.0.1:8000")
    .unwrap()
    .start();
}
