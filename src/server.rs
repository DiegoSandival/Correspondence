use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::transport::{decode_command, encode_response, execute_command, RawResponse};
use crate::{DatabaseRegistry, Result};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");

#[derive(Clone)]
pub struct WebServerConfig {
    pub bind_addr: SocketAddr,
    pub data_root: PathBuf,
    pub index_html: Option<&'static str>,
    pub app_js: Option<&'static str>,
}

impl WebServerConfig {
    pub fn new(bind_addr: SocketAddr, data_root: impl Into<PathBuf>) -> Self {
        Self {
            bind_addr,
            data_root: data_root.into(),
            index_html: None,
            app_js: None,
        }
    }

    pub fn with_index_html(mut self, index_html: &'static str) -> Self {
        self.index_html = Some(index_html);
        self
    }

    pub fn with_app_js(mut self, app_js: &'static str) -> Self {
        self.app_js = Some(app_js);
        self
    }
}

#[derive(Clone)]
struct AppState {
    registry: Arc<Mutex<DatabaseRegistry>>,
    index_html: &'static str,
    app_js: &'static str,
}

pub async fn serve_websocket(config: WebServerConfig) -> Result<()> {
    let state = AppState {
        registry: Arc::new(Mutex::new(DatabaseRegistry::new(&config.data_root)?)),
        index_html: config.index_html.unwrap_or(INDEX_HTML),
        app_js: config.app_js.unwrap_or(APP_JS),
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/app.js", get(app_js_handler))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index_handler(State(state): State<AppState>) -> impl IntoResponse {
    Html(state.index_html)
}

async fn app_js_handler(State(state): State<AppState>) -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        state.app_js,
    )
        .into_response()
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        handle_socket(socket, state).await;
    })
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(message) = socket.next().await {
        let response = match message {
            Ok(Message::Binary(bytes)) => match decode_command(bytes.as_ref()) {
                Ok(command) => {
                    let mut registry = state.registry.lock().await;
                    execute_command(&mut registry, command)
                }
                Err(error) => RawResponse::Error {
                    message: error.to_string(),
                },
            },
            Ok(Message::Text(_)) => RawResponse::Error {
                message: "text websocket frames are not supported; use binary frames only".to_string(),
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            Err(error) => RawResponse::Error {
                message: error.to_string(),
            },
        };

        let payload = encode_response(&response);
        if socket.send(Message::Binary(payload.into())).await.is_err() {
            break;
        }
    }
}
