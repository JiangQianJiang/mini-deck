use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use mini_deck::{
    auth::Auth,
    config::Config,
    keyboard::mock::MockKeyboardBackend,
    server::{router, AppState},
};
use std::sync::Arc;
use tower::ServiceExt;
async fn call(
    app: Router,
    method: &str,
    path: &str,
    token: &str,
    body: &str,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap(),
    )
    .await
    .unwrap()
}
async fn setup(
    fail: bool,
) -> (
    tempfile::TempDir,
    Router,
    Arc<MockKeyboardBackend>,
    String,
    AppState,
) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let config = Config::load(&path).await.unwrap();
    let auth = Auth::new().unwrap();
    let code = auth.code.clone();
    let keyboard = Arc::new(MockKeyboardBackend {
        fail,
        ..Default::default()
    });
    let state = AppState::new(config, auth, keyboard.clone(), path);
    let app = router(state.clone());
    let response = call(
        app.clone(),
        "POST",
        "/api/pair",
        "",
        &format!(r#"{{"code":"{code}"}}"#),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (
        dir,
        app,
        keyboard,
        value["token"].as_str().unwrap().into(),
        state,
    )
}
#[tokio::test]
async fn authenticated_end_to_end_and_logout() {
    let (_dir, app, keyboard, token, _state) = setup(false).await;
    for (method, path) in [
        ("GET", "/api/buttons"),
        ("POST", "/api/buttons/0/press"),
        ("PUT", "/api/buttons/0"),
    ] {
        let body = if method == "PUT" {
            r#"{"name":"新","shortcut":"Alt+F4"}"#
        } else {
            ""
        };
        assert_eq!(
            call(app.clone(), method, path, "", body).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        call(
            app.clone(),
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"name":"关闭","shortcut":"Alt+F4"}"#
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(keyboard.calls.lock().unwrap()[0].keys, vec![0x12, 0x73]);
    assert_eq!(
        call(
            app.clone(),
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"name":"坏","shortcut":"shell"}"#
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/99/press", &token, "")
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(app.clone(), "GET", "/api/buttons", &token, "")
            .await
            .status(),
        StatusCode::OK
    );
    call(app.clone(), "POST", "/api/logout", &token, "").await;
    assert_eq!(
        call(app, "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
async fn concurrent_requests_and_busy_cleanup() {
    let (_dir, app, keyboard, token, state) = setup(false).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let app = app.clone();
        let token = token.clone();
        tasks.spawn(async move {
            call(app, "POST", "/api/buttons/0/press", &token, "")
                .await
                .status()
        });
    }
    while let Some(result) = tasks.join_next().await {
        assert_eq!(result.unwrap(), StatusCode::NO_CONTENT);
    }
    assert_eq!(keyboard.calls.lock().unwrap().len(), 32);
    let guard = state.injection.lock().await;
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(guard);
    assert_eq!(
        call(app, "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}
#[tokio::test]
async fn backend_failure_releases_serialization_guard() {
    let (_dir, app, _, token, state) = setup(true).await;
    assert_eq!(
        call(app, "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(state.injection.try_lock().is_ok());
}
#[tokio::test]
async fn disk_failure_keeps_memory_and_keyboard_usable() {
    let (_dir, app, _, token, state) = setup(false).await;
    tokio::fs::write(state.path.with_extension("json.tmp"), b"x")
        .await
        .unwrap();
    assert_eq!(
        call(
            app.clone(),
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"name":"不保存","shortcut":"Ctrl+Q"}"#
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(state.config.lock().await.buttons[0].name, "复制");
    assert_eq!(
        call(app, "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}
#[tokio::test]
async fn cross_site_and_body_limit() {
    let (_dir, app, _, _, _) = setup(false).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .header("sec-fetch-site", "cross-site")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        call(app, "POST", "/api/pair", "", &"x".repeat(5000))
            .await
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
#[test]
fn pairing_limits_and_revocation() {
    let mut auth = Auth::new().unwrap();
    let code = auth.code.clone();
    for _ in 0..5 {
        assert!(auth.pair("wrong").is_err());
    }
    assert!(auth.pair(&code).is_err());
    let mut auth = Auth::new().unwrap();
    let code = auth.code.clone();
    let token = auth.pair(&code).unwrap();
    assert!(auth.valid(&token));
    auth.logout(&token);
    assert!(!auth.valid(&token));
    for _ in 0..16 {
        auth.pair(&code).unwrap();
    }
    assert!(auth.pair(&code).is_err());
}
