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
├── web/{index.html,style.css,protocol.js,app.js}
├── tests/{api.rs,config.rs,shortcuts.rs,web.cjs,frontend.cjs}
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

本次 macOS 验证：35 个 Rust 测试、11 个 JavaScript 测试通过；`cargo fmt --check`、macOS 与 Windows MSVC 目标 Clippy（`-D warnings`）、Windows 目标全 targets 编译检查及 JavaScript 语法检查通过。macOS Debug Mock 程序的真实 TCP / SSE 与伪终端验证通过，覆盖运行中刷新配对码、旧码失效、已有会话保留，以及带 SSE 连接的 Ctrl+C 退出。

Rust 测试同时覆盖原有快捷键解析、Win32 事件序列、部分注入清理、配置原子替换和认证持久化。测试不向 Windows 注入真实按键。Windows 冒烟脚本已按新 revision 协议更新，本次未在 macOS 执行 PowerShell 脚本。

仍需目标 Windows 电脑验证：真实键盘/UIPI/物理按键干扰、手机扫码和浏览器本地存储、睡眠恢复/网络变更、防火墙、磁盘/断电恢复、24 小时稳定性、CPU/工作集/私有内存/延迟。后续以测量结果决定连接数限制、空闲连接超时及编译参数，暂不引入 TLS、托盘、服务框架或自动更新进程。

## 配置存储实现

配置 JSON 顶层 `buttons`，每项 `id/name/shortcut/position/revision`。固定九个按钮，ID 不重复，位置唯一且为 0–8。名称 1–40 字，快捷键最多 80 字节，请求体最多 4 KiB。启动缺少文件时创建默认配置，解析失败报错退出，保留原数据。

修改时在目标目录创建随机且互不冲突的临时文件，写入并同步后原子替换；阻塞文件操作在惰性阻塞任务内完成。使用 `tempfile` 处理跨平台替换与 RAII 清理，普通失败只删除本次创建的文件，不误删其他文件。旧 `.json.tmp` 不再阻塞保存，也不会被盲目当成配置恢复。强制结束进程或断电仍可能遗留新的临时文件，但不会影响下次保存；需要清理时先确认没有实例运行。不同文件系统和断电行为不提供绝对持久性保证。

不要同时运行多个实例共享同一配置或设备文件；不支持外部热编辑。配置与设备记录只在修改时写入，空闲不落盘。

## 本次一致性与交互改动的验证

开发环境可额外运行以下检查，Node.js 仅用于开发测试，不属于 Windows 运行时依赖：

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
node --check web/app.js
node --check web/protocol.js
node --test tests/web.cjs tests/frontend.cjs
```

Rust 测试补充独立按钮版本、旧配置兼容、过期触发拒绝、同按钮并发编辑冲突、不同按钮独立编辑、磁盘保存失败不发布通知、请求取消后的事务完成、SSE Bearer 认证/容量/慢客户端合并/重连/撤销，以及配对码刷新保持会话与限流。并发注入测试让第一个请求实际等待配置锁，再验证另一个请求被拒绝，避免把同步 Mock 的顺序执行当成并发覆盖。

前端测试直接运行页面脚本，以可控 DOM 和网络验证过期响应、SSE 重连与认证失效、连接取消、重试上限、冲突草稿保留、按键不自动重发、录制生命周期。纯逻辑测试覆盖 UTF-8/事件任意分片、换行、事件大小上限和录制键位映射。它们不替代真实浏览器对 dialog 焦点、移动端布局、输入法及系统保留快捷键的兼容性验证。

本次 Windows 目标检查使用 macOS 上的 `cargo check --all-targets --target x86_64-pc-windows-msvc` 和该目标的 Clippy，仅验证条件编译、类型检查与静态诊断。Windows EXE 链接、真实注入、控制台输入和资源占用仍须 Windows 构建及真机确认；历史 0.2.0 的 CI 结果不能当作本次改动的 Windows 执行结果。

导航与编辑键测试补充 PageUp/PageDown、Home/End、Insert/Delete、Backspace 的虚拟键码、扩展标记、释放顺序、缩写兼容，以及网页录制与 API 保存/触发。新增按键仍需在 Windows 目标应用中验证实际效果。
