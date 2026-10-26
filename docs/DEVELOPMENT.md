# 开发与验证

## 构建与测试

Windows 10/11 x64 开发机安装 Rust stable MSVC 和 Visual Studio C++ Build Tools：

```powershell
cargo test --locked
cargo build --release --locked
.\scripts\Test-WindowsSmoke.ps1 -ExePath .\target\release\mini-deck.exe
.\target\release\mini-deck.exe --lan
```

macOS 开发和 Windows 编译检查：

```sh
cargo test --locked
cargo run -- --mock --listen 127.0.0.1:8765 --config ./config.json
rustup target add x86_64-pc-windows-msvc
cargo check --all-targets --locked --target x86_64-pc-windows-msvc
```

macOS `cargo check` 不链接或运行 Windows EXE。GitHub Actions 的 Windows job 执行测试、Release 链接和 Mock HTTP 冒烟验证；`v*` 标签触发专门的发布工作流，验证标签与 Cargo 版本一致，测试通过后发布 ZIP、EXE 和 SHA-256 校验值。推送 `v*` 标签会自动创建对应版本的 GitHub Release。

## 项目结构与验证范围

```text
mini-deck/
├── Cargo.toml / Cargo.lock
├── src/
│   ├── lib.rs / main.rs / server.rs
│   ├── auth.rs / config.rs / storage.rs
│   ├── connection.rs / shortcut.rs
│   └── keyboard/{mod.rs,windows.rs,mock.rs}
├── web/{index.html,style.css,app.js}
├── tests/{api.rs,config.rs,shortcuts.rs}
├── scripts/
│   ├── Measure-MiniDeck.ps1
│   ├── Test-WindowsSmoke.ps1
│   └── 启动局域网.cmd
├── .cargo/config.toml
├── .github/workflows/{windows.yml,release.yml}
├── docs/{API.md,DEVELOPMENT.md,PERFORMANCE.md,SECURITY.md}
├── .gitignore
├── RELEASE_NOTES.md
└── README.md
```

0.2.0 macOS 本机验证：21 个 Rust 自动化测试通过；`cargo fmt --check`、macOS/Windows 目标 Clippy（`-D warnings`）、Windows 目标全 targets 编译检查、macOS Release 构建及 JavaScript 语法检查通过。本机 Release Mock 的真实 HTTP 冒烟验证了二维码、记住设备跨进程恢复、撤销和重启后仍失效。Windows 实际构建和冒烟结果以对应版本 CI 为准。

0.2.0 本机自动测试覆盖：快捷键解析、规范修饰键顺序、Win32 事件序列、部分注入清理、无效配置、旧临时文件、原子替换失败清理和重试、未认证请求、并发按钮请求、失败后的锁释放、限速/过期/会话上限、记住设备重启恢复、撤销持久化和落盘失败回滚。测试不向 Windows 注入真实按键。Windows 冒烟脚本额外覆盖真实 TCP HTTP、二维码页面和 PowerShell 测量报告生成。

仍需目标 Windows 电脑验证：真实键盘/UIPI/物理按键干扰、手机扫码和浏览器本地存储、睡眠恢复/网络变更、防火墙、磁盘/断电恢复、24 小时稳定性、CPU/工作集/私有内存/延迟。后续以测量结果决定连接数限制、空闲连接超时及编译参数，暂不引入 TLS、托盘、服务框架或自动更新进程。

## 配置存储实现

配置 JSON 顶层 `buttons`，每项 `id/name/shortcut/position`。固定九个按钮，ID 不重复，位置唯一且为 0–8。名称 1–40 字，快捷键最多 80 字节，请求体最多 4 KiB。启动缺少文件时创建默认配置，解析失败报错退出，保留原数据。

修改时在目标目录创建随机且互不冲突的临时文件，写入并同步后原子替换；阻塞文件操作在惰性阻塞任务内完成。使用 `tempfile` 处理跨平台替换与 RAII 清理，普通失败只删除本次创建的文件，不误删其他文件。旧 `.json.tmp` 不再阻塞保存，也不会被盲目当成配置恢复。强制结束进程或断电仍可能遗留新的临时文件，但不会影响下次保存；需要清理时先确认没有实例运行。不同文件系统和断电行为不提供绝对持久性保证。

不要同时运行多个实例共享同一配置或设备文件；不支持外部热编辑。配置与设备记录只在修改时写入，空闲不落盘。
