use crate::{auth::Auth, config::Config, keyboard::KeyboardBackend, shortcut::Shortcut};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub auth: Arc<Mutex<Auth>>,
    pub keyboard: Arc<dyn KeyboardBackend>,
    pub injection: Arc<Mutex<()>>,
    pub updates: Arc<Mutex<()>>,
    pub path: PathBuf,
}
impl AppState {
    pub fn new(
        config: Config,
        auth: Auth,
        keyboard: Arc<dyn KeyboardBackend>,
        path: PathBuf,
    ) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            auth: Arc::new(Mutex::new(auth)),
            keyboard,
            injection: Arc::new(Mutex::new(())),
            updates: Arc::new(Mutex::new(())),
            path,
        }
    }
}
type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: impl ToString) -> ApiError {
    (
        status,
        Json(serde_json::json!({"error":message.to_string()})),
    )
}
fn token(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
}
async fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.auth.lock().await.valid(token(headers)) {
        Ok(())
    } else {
        Err(error(StatusCode::UNAUTHORIZED, "请先配对"))
    }
}
pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../web/style.css"),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/app.js"),
                )
            }),
        )
        .route("/api/health", get(health))
        .route("/api/pair", post(pair))
        .route("/api/logout", post(logout))
        .route("/api/buttons", get(buttons))
        .route("/api/buttons/{id}/press", post(press))
        .route("/api/buttons/{id}", put(update))
        .layer(DefaultBodyLimit::max(4096))
        .layer(axum::middleware::from_fn(security_headers))
        .with_state(state)
}
async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    // 不开放 CORS；拒绝浏览器跨站请求，避免配对接口被第三方页面滥用。
    if request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site")
    {
        return error(StatusCode::FORBIDDEN, "拒绝跨站请求").into_response();
    }
    let mut response = next.run(request).await;
    for (name, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(name),
            axum::http::HeaderValue::from_static(value),
        );
    }
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PairRequest {
    code: String,
}
async fn pair(
    State(state): State<AppState>,
    Json(body): Json<PairRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let token = state
        .auth
        .lock()
        .await
        .pair(&body.code)
        .map_err(|e| error(StatusCode::TOO_MANY_REQUESTS, e))?;
    Ok(Json(serde_json::json!({"token":token})))
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers).await?;
    state.auth.lock().await.logout(token(&headers));
    Ok(StatusCode::NO_CONTENT)
}
async fn buttons(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Config>, ApiError> {
    authorize(&state, &headers).await?;
    Ok(Json(state.config.lock().await.clone()))
}
async fn press(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<u8>,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers).await?;
    // 忙时直接拒绝，避免无限排队和延迟执行旧请求。
    let _guard = state
        .injection
        .try_lock()
        .map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "键盘正忙，请重试"))?;
    let shortcut = {
        let config = state.config.lock().await;
        let button = config
            .buttons
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "按钮不存在"))?;
        Shortcut::parse(&button.shortcut)
            .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?
    };
    state
        .keyboard
        .send_shortcut(&shortcut)
        .map_err(|e| error(StatusCode::SERVICE_UNAVAILABLE, e))?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    name: String,
    shortcut: String,
}
async fn update(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<u8>,
    Json(body): Json<Update>,
) -> Result<StatusCode, ApiError> {
    authorize(&state, &headers).await?;
    let _update = state.updates.lock().await;
    let mut updated = state.config.lock().await.clone();
    let button = updated
        .buttons
        .iter_mut()
        .find(|b| b.id == id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "按钮不存在"))?;
    button.name = body.name;
    button.shortcut = body.shortcut;
    updated
        .validate()
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    updated
        .save(&state.path)
        .await
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    *state.config.lock().await = updated;
    Ok(StatusCode::NO_CONTENT)
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"ok":true,"platform":std::env::consts::OS,"mock":state.keyboard.is_mock()}),
    )
}
