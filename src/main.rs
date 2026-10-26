use mini_deck::{
    auth::Auth,
    config::Config,
    keyboard::KeyboardBackend,
    server::{router, AppState},
};
use std::{
    io::{BufRead, IsTerminal},
    path::PathBuf,
    sync::Arc,
};
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("启动或运行失败：{error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let Options {
        listen,
        path,
        mock,
        clear_devices,
        help,
    } = parse_args(
        std::env::args().skip(1),
        std::env::current_exe()?.with_file_name("config.json"),
    )?;
    if help {
        println!("Mini Deck\n默认监听全部 IPv4 网卡：0.0.0.0:8765\n--local 仅本机访问（127.0.0.1:8765）\n--lan 监听全部 IPv4 网卡（兼容旧启动脚本）\n--listen IP:端口 指定监听地址\n--config 文件路径\n--mock 仅模拟，不注入键盘\n--clear-devices 清空已记住的设备后退出（请先停止旧实例）\n多个监听参数按从左到右处理，最后一个生效。\n运行时在本机控制台输入 r 并回车，刷新配对码");
        return Ok(());
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
        "电脑打开 http://{}/connect 查看连接二维码。",
        std::net::SocketAddr::new(local_ip, bound.port())
    );
    let mut state = AppState::new(config, auth, keyboard, path);
    state.connect_page = Arc::new(mini_deck::connection::page(&urls)?);
    start_console(&state);
    let shutdown = state.shutdown.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            shutdown.send_replace(true);
        })
        .await?;
    Ok(())
}

struct Options {
    listen: String,
    path: PathBuf,
    mock: bool,
    clear_devices: bool,
    help: bool,
}
fn parse_args(
    args: impl IntoIterator<Item = String>,
    default_path: PathBuf,
) -> Result<Options, String> {
    let mut listen = "0.0.0.0:8765".to_string();
    let mut path = default_path;
    let mut mock = false;
    let mut clear_devices = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("缺少监听地址")?,
            "--config" => path = PathBuf::from(args.next().ok_or("缺少配置路径")?),
            "--mock" => mock = true,
            "--lan" => listen = "0.0.0.0:8765".into(),
            "--local" => listen = "127.0.0.1:8765".into(),
            "--clear-devices" => clear_devices = true,
            "--help" => {
                return Ok(Options {
                    listen,
                    path,
                    mock,
                    clear_devices,
                    help: true,
                })
            }
            _ => return Err(format!("未知参数：{arg}")),
        }
    }
    Ok(Options {
        listen,
        path,
        mock,
        clear_devices,
        help: false,
    })
}

fn start_console(state: &AppState) {
    // 只接受交互式本机终端，不提供网络刷新接口；阻塞输入不占用事件循环。
    if !std::io::stdin().is_terminal() {
        return;
    }
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else {
                break;
            };
            if line.trim().eq_ignore_ascii_case("r") && sender.blocking_send(()).is_err() {
                break;
            }
        }
    });
    let state = state.clone();
    println!("本机控制台输入 r 并回车可刷新配对码，已有会话不受影响。");
    tokio::spawn(async move {
        while receiver.recv().await.is_some() {
            let _guard = state.auth_updates.lock().await;
            let mut auth = state.auth.lock().await;
            match auth.refresh_code() {
                Ok(()) => println!("新配对码：{}（10 分钟有效，旧码已失效）", auth.code),
                Err(error) => eprintln!("配对码刷新失败：{error}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, String> {
        parse_args(
            args.iter().map(|s| s.to_string()),
            PathBuf::from("config.json"),
        )
    }
    #[test]
    fn default_is_lan_and_local_is_explicit() {
        assert_eq!(parse(&[]).unwrap().listen, "0.0.0.0:8765");
        assert_eq!(parse(&["--local"]).unwrap().listen, "127.0.0.1:8765");
        assert_eq!(parse(&["--lan"]).unwrap().listen, "0.0.0.0:8765");
    }
    #[test]
    fn last_listen_option_wins_and_other_options_are_preserved() {
        assert_eq!(
            parse(&["--local", "--listen", "192.168.1.20:9000"])
                .unwrap()
                .listen,
            "192.168.1.20:9000"
        );
        assert_eq!(
            parse(&["--listen", "[::1]:9000", "--local"])
                .unwrap()
                .listen,
            "127.0.0.1:8765"
        );
        assert_eq!(parse(&["--local", "--lan"]).unwrap().listen, "0.0.0.0:8765");
        let options = parse(&[
            "--local",
            "--mock",
            "--config",
            "custom.json",
            "--clear-devices",
        ])
        .unwrap();
        assert!(options.mock && options.clear_devices);
        assert_eq!(options.path, PathBuf::from("custom.json"));
        assert!(parse(&["--listen"]).is_err());
        assert!(parse(&["--config"]).is_err());
        assert!(parse(&["--unknown"]).is_err());
        assert!(parse(&["--help"]).unwrap().help);
    }
}
