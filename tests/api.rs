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
    let body = if path.ends_with("/press") && body.is_empty() {
        r#"{"revision":0}"#
    } else {
        body
    };
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
            r#"{"revision":0,"name":"新","shortcut":"Alt+F4"}"#
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
            r#"{"revision":0,"name":"关闭","shortcut":"Alt+F4"}"#
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            app.clone(),
            "POST",
            "/api/buttons/0/press",
            &token,
            r#"{"revision":1}"#
        )
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
            r#"{"revision":1,"name":"坏","shortcut":"shell"}"#
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
    // 让第一个请求真实停在配置读取处，验证其他请求不会与它交错。
    let config = state.config.lock().await;
    let task_app = app.clone();
    let task_token = token.clone();
    let pending = tokio::spawn(async move {
        call(task_app, "POST", "/api/buttons/0/press", &task_token, "").await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if state.injection.try_lock().is_err() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(keyboard.calls.lock().unwrap().is_empty());
    drop(config);
    assert_eq!(pending.await.unwrap().status(), StatusCode::NO_CONTENT);
    assert!(state.injection.try_lock().is_ok());
    assert_eq!(keyboard.calls.lock().unwrap().len(), 1);
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
    let original = tokio::fs::read(&state.path).await.unwrap();
    tokio::fs::remove_file(&state.path).await.unwrap();
    tokio::fs::create_dir(&state.path).await.unwrap();
    assert_eq!(
        call(
            app.clone(),
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"revision":0,"name":"不保存","shortcut":"Ctrl+Q"}"#
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(state.config.lock().await.buttons[0].name, "复制");
    tokio::fs::remove_dir(&state.path).await.unwrap();
    tokio::fs::write(&state.path, original).await.unwrap();
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

async fn response_json(response: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
#[tokio::test]
async fn remembered_device_restart_and_revoke() {
    let (_dir, app, keyboard, temporary, state) = setup(false).await;
    let code = state.auth.lock().await.code.clone();
    let response = call(
        app.clone(),
        "POST",
        "/api/pair",
        "",
        &format!(r#"{{"code":"{code}","remember":true,"name":"平板"}}"#),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = response_json(response).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let sessions = mini_deck::auth::session_path(&state.path);
    let saved = tokio::fs::read_to_string(&sessions).await.unwrap();
    assert!(!saved.contains(&token));
    assert!(!saved.contains(&temporary));
    let restarted = AppState::new(
        Config::load(&state.path).await.unwrap(),
        Auth::load(&sessions).await.unwrap(),
        keyboard,
        state.path.clone(),
    );
    let app = router(restarted);
    assert_eq!(
        call(app.clone(), "GET", "/api/buttons", &temporary, "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = call(app.clone(), "GET", "/api/devices", &token, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let devices = response_json(response).await;
    assert_eq!(devices.as_array().unwrap().len(), 1);
    assert_eq!(devices[0]["current"], true);
    assert!(devices[0].get("digest").is_none());
    let id = devices[0]["id"].as_str().unwrap();
    assert_eq!(
        call(
            app.clone(),
            "DELETE",
            &format!("/api/devices/{id}"),
            &token,
            ""
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(app, "GET", "/api/buttons", &token, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(!Auth::load(&sessions).await.unwrap().valid(&token));
}
#[tokio::test]
async fn remembered_pairing_disk_failure_rolls_back_and_retry_succeeds() {
    let (_dir, app, _, token, state) = setup(false).await;
    let path = mini_deck::auth::session_path(&state.path);
    tokio::fs::create_dir(&path).await.unwrap();
    let code = state.auth.lock().await.code.clone();
    let body = format!(r#"{{"code":"{code}","remember":true,"name":"手机"}}"#);
    assert_eq!(
        call(app.clone(), "POST", "/api/pair", "", &body)
            .await
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(state.auth.lock().await.devices(&token).len(), 1);
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    tokio::fs::remove_dir(&path).await.unwrap();
    assert_eq!(
        call(app, "POST", "/api/pair", "", &body).await.status(),
        StatusCode::OK
    );
}
#[tokio::test]
async fn failed_revoke_preserves_auth_until_durably_saved() {
    let (_dir, app, _, _temporary, state) = setup(false).await;
    let code = state.auth.lock().await.code.clone();
    let token = state
        .auth
        .lock()
        .await
        .pair_device(&code, true, "记住设备")
        .unwrap();
    let id = state
        .auth
        .lock()
        .await
        .devices(&token)
        .into_iter()
        .find(|d| d.current)
        .unwrap()
        .id;
    let path = mini_deck::auth::session_path(&state.path);
    tokio::fs::create_dir(&path).await.unwrap();
    assert_eq!(
        call(
            app.clone(),
            "DELETE",
            &format!("/api/devices/{id}"),
            &token,
            ""
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert!(state.auth.lock().await.valid(&token));
    tokio::fs::remove_dir(&path).await.unwrap();
    assert_eq!(
        call(app, "DELETE", &format!("/api/devices/{id}"), &token, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}
#[tokio::test]
async fn device_management_requires_authentication() {
    let (_dir, app, _, _, _) = setup(false).await;
    assert_eq!(
        call(app.clone(), "GET", "/api/devices", "", "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(app, "DELETE", "/api/devices/test", "", "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn stale_press_and_concurrent_edits_never_execute_or_overwrite() {
    let (_dir, app, keyboard, token, state) = setup(false).await;
    let first = call(
        app.clone(),
        "PUT",
        "/api/buttons/0",
        &token,
        r#"{"revision":0,"name":"甲","shortcut":"Ctrl+Q"}"#,
    );
    let second = call(
        app.clone(),
        "PUT",
        "/api/buttons/0",
        &token,
        r#"{"revision":0,"name":"乙","shortcut":"Ctrl+W"}"#,
    );
    let (first, second) = tokio::join!(first, second);
    let statuses = [first.status(), second.status()];
    assert!(statuses.contains(&StatusCode::NO_CONTENT));
    assert!(statuses.contains(&StatusCode::CONFLICT));
    assert_eq!(
        call(app.clone(), "POST", "/api/buttons/0/press", &token, "")
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert!(keyboard.calls.lock().unwrap().is_empty());
    let saved = Config::load(&state.path).await.unwrap();
    assert_eq!(saved.buttons[0].revision, 1);
    assert_eq!(saved.buttons[1].revision, 0);
    assert_eq!(
        call(
            app,
            "POST",
            "/api/buttons/0/press",
            &token,
            r#"{"revision":1}"#
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(keyboard.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn press_checks_version_and_snapshot_under_the_same_lock() {
    let (_dir, app, keyboard, token, state) = setup(false).await;
    let mut config = state.config.lock().await;
    let task_app = app.clone();
    let task_token = token.clone();
    let pending = tokio::spawn(async move {
        call(task_app, "POST", "/api/buttons/0/press", &task_token, "").await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while state.injection.try_lock().is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    config.buttons[0].revision = 1;
    config.buttons[0].shortcut = "Alt+F4".into();
    drop(config);
    assert_eq!(pending.await.unwrap().status(), StatusCode::CONFLICT);
    assert!(keyboard.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn temporary_logout_and_revoke_ignore_disk_failure() {
    for logout in [true, false] {
        let (_dir, app, _, token, state) = setup(false).await;
        let id = state.auth.lock().await.devices(&token)[0].id.clone();
        tokio::fs::create_dir(mini_deck::auth::session_path(&state.path))
            .await
            .unwrap();
        let path = if logout {
            "/api/logout".into()
        } else {
            format!("/api/devices/{id}")
        };
        assert_eq!(
            call(
                app,
                if logout { "POST" } else { "DELETE" },
                &path,
                &token,
                ""
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );
        assert!(!state.auth.lock().await.valid(&token));
    }
}

async fn next_event(body: &mut Body) -> String {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}
#[tokio::test]
async fn sse_auth_reconnect_coalescing_and_revocation() {
    let (_dir, app, _, token, state) = setup(false).await;
    assert_eq!(
        call(app.clone(), "GET", "/api/events", "", "")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = call(app.clone(), "GET", "/api/events", &token, "").await;
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body();
    assert!(next_event(&mut body).await.contains("event: config"));
    // 故意不消费流，连续发布多次更新：只保留变化信号，随后读取完整快照。
    for revision in 0..3 {
        let request = format!(r#"{{"revision":{revision},"name":"更新","shortcut":"Ctrl+Q"}}"#);
        assert_eq!(
            call(app.clone(), "PUT", "/api/buttons/0", &token, &request)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
    }
    let event = next_event(&mut body).await;
    assert!(!event.contains(&token));
    assert_eq!(
        response_json(call(app.clone(), "GET", "/api/buttons", &token, "").await).await["buttons"]
            [0]["revision"],
        3
    );
    drop(body);
    assert_eq!(state.streams.available_permits(), 32);
    let mut reconnected = call(app.clone(), "GET", "/api/events", &token, "")
        .await
        .into_body();
    assert!(next_event(&mut reconnected).await.contains("event: config"));
    call(app.clone(), "POST", "/api/logout", &token, "").await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), reconnected.frame())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        call(app, "GET", "/api/events", &token, "").await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn failed_save_does_not_publish_revision_or_notification() {
    let (_dir, app, _, token, state) = setup(false).await;
    let receiver = state.changes.subscribe();
    tokio::fs::remove_file(&state.path).await.unwrap();
    tokio::fs::create_dir(&state.path).await.unwrap();
    assert_eq!(
        call(
            app,
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"revision":0,"name":"失败","shortcut":"Ctrl+Q"}"#
        )
        .await
        .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(state.config.lock().await.buttons[0].revision, 0);
    assert!(!receiver.has_changed().unwrap());
}

#[tokio::test]
async fn different_buttons_edit_independently_and_missing_revision_is_rejected() {
    let (_dir, app, keyboard, token, state) = setup(false).await;
    let body = r#"{"revision":0,"name":"独立编辑","shortcut":"Ctrl+Q"}"#;
    let (first, second) = tokio::join!(
        call(app.clone(), "PUT", "/api/buttons/0", &token, body),
        call(app.clone(), "PUT", "/api/buttons/1", &token, body)
    );
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    assert_eq!(second.status(), StatusCode::NO_CONTENT);
    assert_eq!(state.config.lock().await.buttons[1].revision, 1);
    assert_eq!(
        call(app, "POST", "/api/buttons/0/press", &token, "{}")
            .await
            .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(keyboard.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn accepted_edit_transaction_survives_request_cancellation() {
    let (_dir, app, _, token, state) = setup(false).await;
    let guard = state.updates.lock().await;
    let pending = tokio::spawn(async move {
        call(
            app,
            "PUT",
            "/api/buttons/0",
            &token,
            r#"{"revision":0,"name":"不中断","shortcut":"Ctrl+Q"}"#,
        )
        .await
    });
    // 等待事务真正排入更新锁：取消外部 HTTP future 后，内部事务仍须完成。
    tokio::task::yield_now().await;
    pending.abort();
    let _ = pending.await;
    drop(guard);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if state.config.lock().await.buttons[0].revision == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        Config::load(&state.path).await.unwrap().buttons[0].revision,
        1
    );
}

#[tokio::test]
async fn sse_capacity_is_bounded_and_query_credentials_are_rejected() {
    let (_dir, app, _, token, state) = setup(false).await;
    assert_eq!(
        call(
            app.clone(),
            "GET",
            &format!("/api/events?token={token}"),
            "",
            ""
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut bodies = Vec::new();
    for _ in 0..32 {
        bodies.push(
            call(app.clone(), "GET", "/api/events", &token, "")
                .await
                .into_body(),
        );
    }
    assert_eq!(
        call(app.clone(), "GET", "/api/events", &token, "")
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(bodies);
    assert_eq!(state.streams.available_permits(), 32);
    assert_eq!(
        call(app, "GET", "/api/events", &token, "").await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn shutdown_terminates_open_sse_and_releases_permit() {
    let (_dir, app, _, token, state) = setup(false).await;
    let mut body = call(app, "GET", "/api/events", &token, "")
        .await
        .into_body();
    next_event(&mut body).await;
    state.shutdown.send_replace(true);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(state.streams.available_permits(), 32);
}

#[tokio::test]
async fn remembered_logout_failure_preserves_session_and_concurrent_auth_changes_survive() {
    let (_dir, app, _, temporary, state) = setup(false).await;
    let code = state.auth.lock().await.code.clone();
    let request = format!(r#"{{"code":"{code}","remember":true,"name":"记住"}}"#);
    let permanent = response_json(call(app.clone(), "POST", "/api/pair", "", &request).await).await
        ["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = mini_deck::auth::session_path(&state.path);
    tokio::fs::remove_file(&path).await.unwrap();
    tokio::fs::create_dir(&path).await.unwrap();
    assert_eq!(
        call(app.clone(), "POST", "/api/logout", &permanent, "")
            .await
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert!(state.auth.lock().await.valid(&permanent));
    // 设备文件坏掉也不影响另一临时设备注销；保留记住设备集合。
    assert_eq!(
        call(app.clone(), "POST", "/api/logout", &temporary, "")
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
    assert!(state.auth.lock().await.valid(&permanent));
    tokio::fs::remove_dir(&path).await.unwrap();
    let (logout, pair) = tokio::join!(
        call(app.clone(), "POST", "/api/logout", &permanent, ""),
        call(app.clone(), "POST", "/api/pair", "", &request)
    );
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(pair.status(), StatusCode::OK);
    let new_token = response_json(pair).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut auth = state.auth.lock().await;
    assert!(!auth.valid(&permanent));
    assert!(auth.valid(&new_token));
    drop(auth);
    let mut restored = Auth::load(&path).await.unwrap();
    assert!(!restored.valid(&permanent));
    assert!(restored.valid(&new_token));
}

#[tokio::test]
async fn navigation_shortcuts_can_be_saved_and_triggered() {
    let (_dir, app, keyboard, token, _state) = setup(false).await;
    for (revision, (name, key)) in [
        ("PageUp", 0x21),
        ("PageDown", 0x22),
        ("Home", 0x24),
        ("End", 0x23),
        ("Insert", 0x2d),
        ("Delete", 0x2e),
        ("Backspace", 0x08),
    ]
    .into_iter()
    .enumerate()
    {
        let body = format!(r#"{{"revision":{revision},"name":"导航","shortcut":"Ctrl+{name}"}}"#);
        assert_eq!(
            call(app.clone(), "PUT", "/api/buttons/0", &token, &body)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        let body = format!(r#"{{"revision":{}}}"#, revision + 1);
        assert_eq!(
            call(app.clone(), "POST", "/api/buttons/0/press", &token, &body)
                .await
                .status(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            keyboard.calls.lock().unwrap().last().unwrap().keys,
            vec![0x11, key]
        );
    }
}
