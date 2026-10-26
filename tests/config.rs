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
async fn stale_file_does_not_block_saving_or_get_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let mut config = Config::load(&path).await.unwrap();
    let stale = path.with_extension("json.tmp");
    tokio::fs::write(&stale, "旧版本残留").await.unwrap();
    config.buttons[0].name = "新名称".into();
    config.save(&path).await.unwrap();
    assert_eq!(Config::load(&path).await.unwrap().buttons[0].name, "新名称");
    assert_eq!(
        tokio::fs::read_to_string(&stale).await.unwrap(),
        "旧版本残留"
    );
}
#[tokio::test]
async fn failed_replace_cleans_owned_temporary_file_and_can_retry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    tokio::fs::create_dir(&path).await.unwrap();
    assert!(Config::default().save(&path).await.is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    tokio::fs::remove_dir(&path).await.unwrap();
    Config::default().save(&path).await.unwrap();
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
