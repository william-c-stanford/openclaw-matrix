pub mod config;
pub mod device;
pub mod protocol;

use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use self::config::GatewayConfig;
use self::device::DeviceIdentity;
use self::protocol::{IncomingFrame, RequestFrame, build_auth_respond, build_chat_send};

/// Commands sent TO the gateway task
#[derive(Debug)]
#[allow(dead_code)]
pub enum GatewayCommand {
    SendMessage(String),
    Disconnect,
}

/// Actions received FROM the gateway task
#[derive(Debug)]
pub enum GatewayAction {
    Connected,
    Disconnected(String),
    ChatDelta(String),
    ChatComplete(String),
    Error(String),
    MoodUpdate(crate::mood::MoodUpdate),
}

/// Connection status for UI display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

/// Spawn the gateway WebSocket task.
/// Returns (command_sender, action_receiver, initial_status).
pub fn spawn_gateway(
    gateway_config: GatewayConfig,
) -> (mpsc::Sender<GatewayCommand>, mpsc::Receiver<GatewayAction>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<GatewayCommand>(32);
    let (act_tx, act_rx) = mpsc::channel::<GatewayAction>(64);

    tokio::spawn(gateway_task(gateway_config, cmd_rx, act_tx));

    (cmd_tx, act_rx)
}

async fn gateway_task(
    config: GatewayConfig,
    mut cmd_rx: mpsc::Receiver<GatewayCommand>,
    act_tx: mpsc::Sender<GatewayAction>,
) {
    let mut request_id: u64 = 1;
    let mut backoff_ms: u64 = 1000;

    loop {
        // Connect
        let connect_result = tokio_tungstenite::connect_async(&config.url).await;
        let (ws_stream, _) = match connect_result {
            Ok(conn) => {
                backoff_ms = 1000; // reset on successful connect
                conn
            }
            Err(e) => {
                let _ = act_tx
                    .send(GatewayAction::Disconnected(format!("connect failed: {e}")))
                    .await;
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(30_000);
                continue;
            }
        };

        let (mut ws_write, mut ws_read) = ws_stream.split();

        // Load device identity for auth
        let identity = match DeviceIdentity::load_or_create() {
            Ok(id) => id,
            Err(e) => {
                let _ = act_tx
                    .send(GatewayAction::Error(format!("device identity error: {e}")))
                    .await;
                break;
            }
        };

        // Main message loop
        let mut authenticated = false;
        loop {
            tokio::select! {
                // Incoming WebSocket messages
                msg = ws_read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            match IncomingFrame::parse(&text) {
                                IncomingFrame::Challenge { challenge } => {
                                    let sig = identity.sign(&challenge);
                                    let frame = RequestFrame::new(
                                        request_id,
                                        "auth.respond",
                                        Some(build_auth_respond(&identity.device_id, &sig)),
                                    );
                                    request_id += 1;
                                    let _ = ws_write.send(Message::Text(frame.to_json().into())).await;
                                }
                                IncomingFrame::Hello => {
                                    authenticated = true;
                                    let _ = act_tx.send(GatewayAction::Connected).await;
                                }
                                IncomingFrame::ChatDelta { delta } => {
                                    let _ = act_tx.send(GatewayAction::ChatDelta(delta)).await;
                                }
                                IncomingFrame::ChatComplete { content } => {
                                    let _ = act_tx.send(GatewayAction::ChatComplete(content)).await;
                                }
                                IncomingFrame::Error { code, message } => {
                                    let _ = act_tx.send(GatewayAction::Error(format!("[{code}] {message}"))).await;
                                }
                                IncomingFrame::MoodUpdate(update) => {
                                    let _ = act_tx.send(GatewayAction::MoodUpdate(update)).await;
                                }
                                IncomingFrame::Unknown(_) => {}
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            break; // Connection closed, will reconnect
                        }
                        Some(Err(e)) => {
                            let _ = act_tx
                                .send(GatewayAction::Disconnected(format!("ws error: {e}")))
                                .await;
                            break;
                        }
                        _ => {}
                    }
                }
                // Outgoing commands from app
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(GatewayCommand::SendMessage(content)) => {
                            if authenticated {
                                let frame = RequestFrame::new(
                                    request_id,
                                    "chat.send",
                                    Some(build_chat_send(&content)),
                                );
                                request_id += 1;
                                let _ = ws_write.send(Message::Text(frame.to_json().into())).await;
                            }
                        }
                        Some(GatewayCommand::Disconnect) | None => {
                            let _ = ws_write.close().await;
                            let _ = act_tx.send(GatewayAction::Disconnected("user disconnect".into())).await;
                            return; // Exit task entirely
                        }
                    }
                }
            }
        }

        // Connection lost, reconnect after backoff
        let _ = act_tx
            .send(GatewayAction::Disconnected("connection lost, reconnecting...".into()))
            .await;
        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(30_000);
    }
}
