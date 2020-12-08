use anyhow::{bail, Result};

use pueue::message::*;
use pueue::platform::socket::get_client;
use pueue::platform::socket::Socket;
use pueue::protocol::*;
use pueue::settings::Settings as PueueSettings;
use pueue::state::State;

use crate::settings::Settings;

pub async fn get_pueue_socket(settings: &Settings) -> Result<Socket> {
    // Try to read settings from the configuration file.
    let pueue_settings = PueueSettings::new(true, &None)?;

    let mut socket = if let Some(port) = &settings.pueue_port {
        get_client(None, Some(port.clone())).await?
    } else if let Some(socket_path) = &settings.pueue_unix_socket {
        get_client(Some(socket_path.clone()), None).await?
    } else {
        bail!("Please either specify a Pueue port or unix socket path.");
    };

    // Send the secret to the daemon
    // In case everything was successful, we get a short `hello` response from the daemon.
    let secret = pueue_settings.shared.secret.clone().into_bytes();
    send_bytes(&secret, &mut socket).await?;
    let hello = receive_bytes(&mut socket).await?;
    if hello != b"hello" {
        bail!("Daemon went away after initial connection. Did you use the correct secret?")
    }

    let _state = get_state(&mut socket).await?;

    Ok(socket)
}

// This is a helper function for easy retrieval of the current daemon state.
// The current daemon state is often needed in more complex commands.
pub async fn get_state(socket: &mut Socket) -> Result<State> {
    // Create the message payload and send it to the daemon.
    send_message(Message::Status, socket).await?;

    // Check if we can receive the response from the daemon
    let message = receive_message(socket).await?;

    match message {
        Message::StatusResponse(state) => Ok(state),
        _ => unreachable!(),
    }
}
