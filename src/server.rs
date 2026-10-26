use crate::{auth::Auth, config::Config, keyboard::KeyboardBackend, shortcut::Shortcut};
use axum::response::sse::{Event, Sse};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{watch, Mutex, Semaphore};
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub auth: Arc<Mutex<Auth>>,
    pub keyboard: Arc<dyn KeyboardBackend>,
    pub injection: Arc<Mutex<()>>,
    pub updates: Arc<Mutex<()>>,
    pub auth_updates: Arc<Mutex<()>>,
    pub connect_page: Arc<String>,
    pub path: PathBuf,
    pub changes: watch::Sender<()>,
    pub shutdown: watch::Sender<bool>,
    pub streams: Arc<Semaphore>,
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
            auth_updates: Arc::new(Mutex::new(())),
            connect_page: Arc::new(String::from("连接信息尚未初始化")),
            path,
            changes: watch::channel(()).0,
            shutdown: watch::channel(false).0,
            streams: Arc::new(Semaphore::new(32)),
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
        .route(
            "/protocol.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/protocol.js"),
                )
            }),
        )
        .route("/connect", get(connect))
        .route("/api/devices", get(devices))
        .route("/api/devices/{id}", axum::routing::delete(revoke))
        .route("/api/health", get(health))
        .route("/api/pair", post(pair))
        .route("/api/logout", post(logout))
        .route("/api/buttons", get(buttons))
        .route("/api/events", get(events))
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
    #[serde(default)]
    remember: bool,
    #[serde(default = "default_device_name")]
    name: String,
}
fn default_device_name() -> String {
    "浏览器设备".into()
}
async fn persist_auth(state: &AppState, auth: &Auth) -> Result<(), ApiError> {
    let bytes = auth
        .snapshot()
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    crate::storage::write_atomic(&crate::auth::session_path(&state.path), bytes)
        .await
        .map_err(|e| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("设备保存失败：{e}"),
            )
        })
}
async fn pair(
    State(state): State<AppState>,
    Json(body): Json<PairRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _update = state.auth_updates.lock().await;
    let (token, snapshot) = {
        let mut auth = state.auth.lock().await;
        let token = auth
            .pair_device(&body.code, body.remember, &body.name)
            .map_err(|e| error(StatusCode::TOO_MANY_REQUESTS, e))?;
        (token, auth.clone())
    };
    if body.remember {
        if let Err(error) = persist_auth(&state, &snapshot).await {
            state.auth.lock().await.logout(&token);
            return Err(error);
        }
    }
    Ok(Json(
        serde_json::json!({"token":token,"remembered":body.remember}),
    ))
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    let _update = state.auth_updates.lock().await;
    authorize(&state, &headers).await?;
    let mut updated = state.auth.lock().await.clone();
    let persistent = updated.remembered_token(token(&headers));
    updated.logout(token(&headers));
    if persistent {
        persist_auth(&state, &updated).await?;
    }
    *state.auth.lock().await = updated;
    state.changes.send_replace(());
    Ok(StatusCode::NO_CONTENT)
}
async fn devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::auth::Device>>, ApiError> {
    authorize(&state, &headers).await?;
    Ok(Json(state.auth.lock().await.devices(token(&headers))))
}
async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _update = state.auth_updates.lock().await;
    authorize(&state, &headers).await?;
    let mut updated = state.auth.lock().await.clone();
    let persistent = updated.remembered_id(&id);
    if !updated.revoke(&id) {
        return Err(error(StatusCode::NOT_FOUND, "设备不存在"));
    }
    if persistent {
        persist_auth(&state, &updated).await?;
    }
    *state.auth.lock().await = updated;
    state.changes.send_replace(());
    Ok(StatusCode::NO_CONTENT)
}
async fn connect(State(state): State<AppState>) -> axum::response::Html<String> {
    axum::response::Html(state.connect_page.as_ref().clone())
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
    Json(body): Json<Revision>,
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
        check_revision(button.revision, body.revision)?;
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
    revision: u32,
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
    // 请求断开也完成已接收的保存事务，避免磁盘已替换但内存版本未发布。
    tokio::spawn(async move {
        let _update = state.updates.lock().await;
        let mut updated = state.config.lock().await.clone();
        let button = updated
            .buttons
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "按钮不存在"))?;
        check_revision(button.revision, body.revision)?;
        button.revision = button
            .revision
            .checked_add(1)
            .ok_or_else(|| error(StatusCode::CONFLICT, "按钮版本已用尽"))?;
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
        state.changes.send_replace(());
        Ok(StatusCode::NO_CONTENT)
    })
    .await
    .map_err(|e| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("配置更新任务失败：{e}"),
        )
    })?
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"ok":true,"platform":std::env::consts::OS,"mock":state.keyboard.is_mock()}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: u32,
}
fn check_revision(actual: u32, expected: u32) -> Result<(), ApiError> {
    if actual != expected {
        return Err(error(
            StatusCode::CONFLICT,
            "按钮配置已被其他设备修改，请刷新后确认；未执行按键或覆盖配置",
        ));
    }
    Ok(())
}
async fn events(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    authorize(&state, &headers).await?;
    let permit = state
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "同步连接已满"))?;
    // watch 只保留一次变化信号，慢客户端不积压；初次/重连始终通知完整刷新。
    let receiver = state.changes.subscribe();
    let shutdown = state.shutdown.subscribe();
    let stream = futures_util::stream::unfold(
        (
            state,
            token(&headers).to_owned(),
            receiver,
            shutdown,
            true,
            permit,
        ),
        |(state, token, mut receiver, mut shutdown, first, permit)| async move {
            if *shutdown.borrow() {
                return None;
            }
            let remaining = state.auth.lock().await.session_remaining(&token)?;
            if !first {
                tokio::select! {
                    _ = shutdown.changed() => { return None; }
                    changed = receiver.changed() => { changed.ok()?; }
                    _ = tokio::time::sleep(remaining.min(Duration::from_secs(30 * 86400))) => {}
                }
            }
            if !state.auth.lock().await.valid(&token) {
                return None;
            }
            let event = Event::default().event("config").data("{}");
            Some((
                Ok::<_, Infallible>(event),
                (state, token, receiver, shutdown, false, permit),
            ))
        },
    );
    Ok(Sse::new(stream).into_response())
}
