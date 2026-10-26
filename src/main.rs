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
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("缺少监听地址")?,
            "--config" => path = PathBuf::from(args.next().ok_or("缺少配置路径")?),
            "--mock" => mock = true,
            "--help" => {
                println!("Mini Deck\n--listen IP:端口（默认 127.0.0.1:8765）\n--config 文件路径\n--mock 仅模拟，不注入键盘");
                return Ok(());
            }
            _ => return Err(format!("未知参数：{arg}").into()),
        }
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
    let auth = Auth::new()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Mini Deck 已启动{}\n监听：http://{}\n配对码：{}（10 分钟有效）\n会话有效 24 小时；重启撤销全部会话。\n配置：{}\n局域网请使用 --listen 电脑的局域网IP:8765；手机访问同一地址。\n仅限可信私人局域网；HTTP 不防窃听。按 Ctrl+C 退出。", if mock { "（模拟模式）" } else { "" },listener.local_addr()?,auth.code,path.display());
    axum::serve(
        listener,
        router(AppState::new(config, auth, keyboard, path)),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
