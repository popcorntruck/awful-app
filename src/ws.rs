use std::collections::HashMap;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition(i64, i64);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserState {
    cursor: CursorPosition,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "d", rename_all = "camelCase")]
pub enum ToClient {
    Identify { id: String },
    UpdateCursors(HashMap<String, UserState>),
    CountChanged(i64),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "d", rename_all = "camelCase")]
pub enum FromClient {
    UpdateMyCursor(CursorPosition),
}

pub async fn handle_socket(ws: WebSocket, state: AppState) {
    let (mut send_ws, mut recv_ws) = ws.split();
    let id = Uuid::new_v4();
    let id_str = id.to_string();
    let mut users = state.0.users.write().await;

    users.insert(
        id_str.clone(),
        UserState {
            cursor: CursorPosition(0, 0),
        },
    );

    drop(users);

    let msg = ToClient::Identify { id: id_str.clone() };
    send_ws
        .send(Message::Text(serde_json::to_string(&msg).unwrap()))
        .await
        .unwrap();

    let mut dispatch_rx = state.0.dispatch.subscribe();

    let recv_dispatch = tokio::spawn(async move {
        while let Ok(msg) = dispatch_rx.recv().await {
            let msg = ToClient::UpdateCursors(msg);

            if let Ok(json) = serde_json::to_string(&msg) {
                //dont care if we fail because it will be send later
                let _ = send_ws.send(Message::Text(json)).await;
            }
        }
    });

    //drop(users);

    info!("Client connected and assigned id {}", id_str);

    while let Some(msg) = recv_ws.next().await {
        if let Ok(msg) = msg {
            if let Message::Text(msg) = msg {
                let data = serde_json::from_str::<FromClient>(&msg);

                //ignore InValid data
                if let Ok(payload) = data {
                    match payload {
                        FromClient::UpdateMyCursor(pos) => {
                            let mut users = state.0.users.write().await;
                            users.entry(id_str.clone()).and_modify(|o| o.cursor = pos);
                            let _ = state.0.dispatch.send(users.clone());
                        }
                    }
                }
            }
        }
    }

    let mut users = state.0.users.write().await;

    info!("Client {} disconnected!", id_str);
    users.remove_entry(&id_str);
    recv_dispatch.abort();

    //send to dispatch so cursor disappaer
    let _ = state.0.dispatch.send(users.clone());
}
