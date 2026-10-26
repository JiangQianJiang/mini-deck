use mini_deck::config::Config;
#[tokio::test]
async fn load_save_and_reject_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let mut config = Config::load(&path).await.unwrap();
    config.buttons[0].name = "测试".into();
    config.save(&path).await.unwrap();
    assert_eq!(Config::load(&path).await.unwrap().buttons[0].name, "测试");
    tokio::fs::write(&path, b"invalid").await.unwrap();
    assert!(Config::load(&path).await.unwrap_err().contains("解析失败"));
}
#[tokio::test]
async fn failed_replace_preserves_previous_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let config = Config::load(&path).await.unwrap();
    tokio::fs::write(path.with_extension("json.tmp"), "残留".as_bytes())
        .await
        .unwrap();
    assert!(config.save(&path).await.is_err());
    assert_eq!(Config::load(&path).await.unwrap().buttons.len(), 9);
}
#[test]
fn invalid_configuration() {
    let mut c = Config::default();
    c.buttons[0].position = 1;
    assert!(c.validate().is_err());
    let mut c = Config::default();
    c.buttons[0].shortcut = "shell".into();
    assert!(c.validate().is_err());
}
