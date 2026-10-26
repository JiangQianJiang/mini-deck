use mini_deck::{
    auth::Auth,
    config::Config,
    keyboard::KeyboardBackend,
    server::{router, AppState},
};
use std::{path::PathBuf, sync::Arc};
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("启动或运行失败：{error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen = "127.0.0.1:8765".to_string();
    let mut path = std::env::current_exe()?.with_file_name("config.json");
    let mut mock = false;
    let mut clear_devices = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("缺少监听地址")?,
            "--config" => path = PathBuf::from(args.next().ok_or("缺少配置路径")?),
            "--mock" => mock = true,
            "--lan" => listen = "0.0.0.0:8765".into(),
            "--clear-devices" => clear_devices = true,
            "--help" => {
                println!("Mini Deck\n--listen IP:端口（默认 127.0.0.1:8765）\n--config 文件路径\n--mock 仅模拟，不注入键盘\n--lan 监听全部 IPv4 网卡（8765）\n--clear-devices 清空已记住的设备后退出（请先停止旧实例）");
                return Ok(());
            }
            _ => return Err(format!("未知参数：{arg}").into()),
        }
    }
    let sessions = mini_deck::auth::session_path(&path);
    if clear_devices {
        mini_deck::storage::write_atomic(&sessions, b"[]".to_vec()).await?;
        println!("已清空设备文件；请确保旧实例已停止，再重新启动程序。");
        return Ok(());
    }
    let address: std::net::SocketAddr = listen.parse()?;
    #[cfg(windows)]
    let keyboard: Arc<dyn KeyboardBackend> = if mock {
        Arc::new(mini_deck::keyboard::mock::MockKeyboardBackend::default())
    } else {
        Arc::new(mini_deck::keyboard::windows::WindowsKeyboardBackend)
    };
    #[cfg(not(windows))]
    let keyboard: Arc<dyn KeyboardBackend> = {
        if !mock {
            return Err("非 Windows 环境必须显式使用 --mock".into());
        }
        Arc::new(mini_deck::keyboard::mock::MockKeyboardBackend::default())
    };
    let config = Config::load(&path).await?;
    let auth = Auth::load(&sessions).await?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    let bound = listener.local_addr()?;
    let urls = match mini_deck::connection::urls(bound) {
        Ok(urls) => urls,
        Err(e) => {
            eprintln!("{e}");
            Vec::new()
        }
    };
    println!("Mini Deck {} 已启动{}\n监听：http://{}\n配对码：{}（10 分钟有效）\n临时会话 24 小时；记住设备 30 天，重启仍有效。\n配置：{}\n仅限可信私人局域网；HTTP 不防窃听。按 Ctrl+C 退出。", env!("CARGO_PKG_VERSION"), if mock { "（模拟模式）" } else { "" }, bound, auth.code, path.display());
    for url in &urls {
        println!("访问地址：{url}");
    }
    let local_ip = if bound.ip().is_unspecified() {
        if bound.is_ipv4() {
            "127.0.0.1".parse()?
        } else {
            "::1".parse()?
        }
    } else {
        bound.ip()
    };
    println!(
        "电脑打开 http://{}/connect 查看连接二维码。\n手机访问需 --lan 或 --listen 局域网IP:8765。",
        std::net::SocketAddr::new(local_ip, bound.port())
    );
    let mut state = AppState::new(config, auth, keyboard, path);
    state.connect_page = Arc::new(mini_deck::connection::page(&urls)?);
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
