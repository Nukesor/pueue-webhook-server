use pueue_lib::{network::message::GroupMessage, prelude::*};

use crate::{internal_prelude::*, settings::Settings as InternalSettings};

pub async fn get_pueue_client(settings: &InternalSettings) -> Result<Client> {
    // Try to read settings from the default configuration file.
    let (pueue_settings, _) = Settings::read(&None)?;

    // Create client to talk with the daemon and connect.
    let mut client = Client::new(pueue_settings, true)
        .await
        .context("Failed to initialize client.")?;

    // Every webhook can run in a separate pueue group.
    // Get the currently available Pueue groups, so we know which groups we have to create.
    let state = get_state(&mut client).await?;
    let mut existing_groups: Vec<String> = state.groups.keys().cloned().collect();

    // Create all missing groups in Pueue.
    for webhook in settings.webhooks.iter() {
        if !existing_groups.contains(&webhook.pueue_group) {
            info!("Create new pueue group {}", webhook.pueue_group);

            let message = Request::Group(GroupMessage::Add {
                name: webhook.pueue_group.clone(),
                parallel_tasks: None,
            });
            client.send_request(message).await?;
            existing_groups.push(webhook.pueue_group.clone());
        }
    }

    Ok(client)
}

// This is a helper function for easy retrieval of the current daemon state.
// The current daemon state is often needed in more complex commands.
pub async fn get_state(client: &mut Client) -> Result<State> {
    // Request the state.
    client.send_request(Request::Status).await?;
    let response = client.receive_response().await?;

    match response {
        Response::Status(state) => Ok(*state),
        _ => unreachable!(),
    }
}
