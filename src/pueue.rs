use anyhow::{bail, Result};
use log::info;
use pueue_lib::network::message::*;
use pueue_lib::network::platform::socket::{get_client_stream, GenericStream};
use pueue_lib::network::protocol::*;
use pueue_lib::network::secret::read_shared_secret;
use pueue_lib::settings::Settings as PueueSettings;
use pueue_lib::state::State;

use crate::settings::Settings;

pub async fn get_pueue_socket(settings: &Settings) -> Result<GenericStream> {
    // Try to read settings from the configuration file.
    let pueue_settings = PueueSettings::new(true, &None)?;

    let mut stream = get_client_stream(&pueue_settings.shared).await?;

    // Send the secret to the daemon
    // In case everything was successful, we get a short `hello` response from the daemon.
    let secret = read_shared_secret(&pueue_settings.shared.shared_secret_path)?;
    send_bytes(&secret, &mut stream).await?;
    let hello = receive_bytes(&mut stream).await?;
    if hello != b"hello" {
        bail!("Daemon went away after initial connection. Did you use the correct secret?")
    }

    // Every webhook can run in a separate pueue group.
    let state = get_state(&mut stream).await?;
    let existing_groups: Vec<String> = state.groups.keys().cloned().collect();

    // Create those groups, if they don't exist yet.
    for webhook in settings.webhooks.iter() {
        if !existing_groups.contains(&webhook.pueue_group) {
            info!("Create new pueue group {}", webhook.pueue_group);
            let add_group_message = GroupMessage {
                add: Some(webhook.pueue_group.clone()),
                remove: None,
            };

            send_message(Message::Group(add_group_message), &mut stream).await?;
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
