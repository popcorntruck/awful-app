mod ws;

use axum::{
    extract::{State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use ws::{handle_socket, UserState};

#[derive(Clone, Debug)]
pub struct AppState(Arc<InnerState>);

pub type UserMap = HashMap<String, UserState>;

#[derive(Debug)]
pub struct InnerState {
    users: RwLock<UserMap>,
    dispatch: broadcast::Sender<UserMap>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (tx, _rx) = broadcast::channel::<UserMap>(1024);

    let state = AppState(Arc::new(InnerState {
        users: RwLock::new(HashMap::new()),
        dispatch: tx,
    }));

    let app = Router::new()
        .route("/", get(root))
        .route("/ws", get(ws_route_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .expect("failed to start axum server");
}

async fn ws_route_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn root() -> impl IntoResponse {
    Html(include_str!("index.html"))
}
