use anyhow::{bail, Result};
use log::info;
use pueue_lib::network::message::*;
use pueue_lib::network::protocol::*;
use pueue_lib::network::secret::read_shared_secret;
use pueue_lib::settings::Settings as PueueSettings;
use pueue_lib::state::State;

use crate::settings::Settings;

pub async fn get_pueue_socket(settings: &Settings) -> Result<GenericStream> {
    // Try to read settings from the configuration file.
    let (pueue_settings, _) = PueueSettings::read(&None)?;

    let mut stream = get_client_stream(&pueue_settings.shared).await?;

    // Send the secret to the daemon
    // In case everything was successful, we get a short `hello` response from the daemon.
    let secret = read_shared_secret(&pueue_settings.shared.shared_secret_path())?;
    send_bytes(&secret, &mut stream).await?;
    let hello = receive_bytes(&mut stream).await?;
    if hello.is_empty() {
        bail!("Daemon went away after initial connection. Did you use the correct secret?")
    }

    // Every webhook can run in a separate pueue group.
    // Get the currently available Pueue groups, so we know which groups we have to create.
    let state = get_state(&mut stream).await?;
    let mut existing_groups: Vec<String> = state.groups.keys().cloned().collect();

    // Create all missing groups in Pueue.
    for webhook in settings.webhooks.iter() {
        if !existing_groups.contains(&webhook.pueue_group) {
            info!("Create new pueue group {}", webhook.pueue_group);

            let message = Message::Group(GroupMessage::Add {
                name: webhook.pueue_group.clone(),
                parallel_tasks: None,
            });
            send_message(message, &mut stream).await?;
            existing_groups.push(webhook.pueue_group.clone());
        }
    }

    Ok(stream)
}

// This is a helper function for easy retrieval of the current daemon state.
// The current daemon state is often needed in more complex commands.
pub async fn get_state(socket: &mut GenericStream) -> Result<State> {
    // Create the message payload and send it to the daemon.
    send_message(Message::Status, socket).await?;

    // Check if we can receive the response from the daemon
    let message = receive_message(socket).await?;

    match message {
        Message::StatusResponse(state) => Ok(*state),
        _ => unreachable!(),
    }
}
