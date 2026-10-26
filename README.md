# Mini Deck

轻量 Windows 局域网快捷键控制器。Rust 原生程序直接提供手机网页，通过 Win32 `SendInput` 向电脑当前前台窗口发送快捷键。没有 Node、Python、浏览器运行时、数据库、插件或后台前端服务。

## 构建与运行

Windows 10/11 x64 开发机安装 Rust stable MSVC 工具链和 Visual Studio C++ Build Tools：

```powershell
cargo test --locked
cargo build --release --locked
.\target\release\mini-deck.exe
```

Windows x64 MSVC 目标通过 `.cargo/config.toml` 静态链接 C 运行库，仍使用 Windows 自带系统 DLL。最终用户只需 EXE，不需要 Rust 等开发环境。默认仅监听 `127.0.0.1:8765`，双击后可在电脑打开该地址；局域网访问要明确指定本机地址，例如：

```powershell
.\mini-deck.exe --listen 192.168.1.20:8765
# 或编辑快捷方式的“目标”，在 EXE 路径后追加 --listen 参数
```

控制台显示监听地址及随机配对码。通过 `ipconfig` 查找当前 Wi-Fi/以太网 IPv4 地址。手机连接同一私人网络，浏览器打开 `http://192.168.1.20:8765`，输入配对码后点击按钮。Windows 防火墙仅允许私人网络上的该程序/端口；不要开放公网转发。无线客户端隔离可能阻止连接。

配置默认位于 EXE 同目录 `config.json`，目录须可写；可用 `--config C:\Users\你的名字\mini-deck.json` 指定路径。`--help` 显示参数。Ctrl+C 关闭。非 Windows 必须传入 `--mock`，模拟模式只记录调用，不执行真实快捷键。

```sh
cargo test --locked
cargo run -- --mock --listen 127.0.0.1:8765 --config ./config.json
cargo check --locked --target x86_64-pc-windows-msvc
```

macOS 上 Windows 目标的 `cargo check` 只检查编译类型，不链接 EXE。真正的 EXE 在 Windows 构建；附带 GitHub Actions Windows 测试/Release 构建工作流，推送至 GitHub 后可下载 artifact。当前没有将此工作流提交到远程或运行。

## 使用和安全边界

默认九宫格：复制、粘贴、剪切、撤销、重做、全选、保存、查找、任务管理器。勾选“编辑按钮”后点击按钮修改名称和快捷键，保存成功后才更新内存配置。主键支持 A–Z、0–9、F1–F12、Enter、Esc、Tab、Space、Left、Right、Up、Down；修饰键 Ctrl、Alt、Shift、Win。不支持多主键、宏或系统命令。

快捷键作用于 **Windows 当前前台窗口**，不是手机上的窗口。请先在电脑选择目标应用。普通权限进程不能可靠控制管理员权限窗口；UIPI 可能拒绝注入且不提供有意义的 GetLastError。锁屏、安全桌面、Ctrl+Alt+Delete 不受支持。必须在已登录的交互式用户会话中运行，不能作为 Session 0 系统服务使用。真实物理按键仍可能影响组合键；程序不会主动释放用户实际按住的键。部分注入失败后会尽力释放本组合键，但系统也可能拒绝清理，不能保证恢复状态。

配对码使用系统随机数生成，32 位随机值，10 分钟后失效。五次错误后全局阻止配对 60 秒，避免按 IP 限速被绕过；这也意味着攻击者可暂时阻止配对。已建立会话不受配对码过期影响。256 位会话令牌最多 16 个，24 小时固定过期；页面只在内存持有令牌，刷新/关闭后需要重新配对。关闭页面不会立即删除服务端会话，请优先点“注销设备”。重启程序撤销全部会话并更换配对码，适用于配对过期或会话已满。

API 使用 `Authorization: Bearer ...`，不会将凭据放到 URL。没有 CORS 开放，无 Cookie 认证；JSON 写入和跨站 Fetch 检查减少第三方网页滥用。静态资源使用 CSP、禁嵌入和禁止缓存头。**HTTP 的配对码、令牌和按键请求可被同网段攻击者窃听、修改或重放，无法抵御中间人攻击。仅适用于可信私人局域网。** 不适合公共 Wi-Fi、企业不可信网络或公网。第一版没有 TLS、连接级抗 DoS 或多用户权限隔离。

## HTTP API

除健康检查、配对和静态页面外，均需要 Bearer 令牌。

| 接口 | 输入及结果 |
| --- | --- |
| `GET /api/health` | `ok` 和 `platform`，不代表真实键盘已验证 |
| `POST /api/pair` | `{"code":"控制台配对码"}` → `{"token":"..."}` |
| `GET /api/buttons` | `{"buttons":[...]}` |
| `POST /api/buttons/{id}/press` | 仅提交 ID；成功 204，键盘失败 503，忙时 429 |
| `PUT /api/buttons/{id}` | `{"name":"复制","shortcut":"Ctrl+C"}`，成功 204 |
| `POST /api/logout` | 撤销当前会话，成功 204 |

配置固定九个按钮，唯一 ID 和位置 0–8；网页只编辑名称和快捷键。名称最多 40 字，快捷键最多 80 字节，请求体最多 4 KiB。配置 JSON 顶层 `buttons`，每项 `id/name/shortcut/position`。无效配置启动时明确报错退出，不覆盖损坏数据。写入同目录临时文件，sync 后安全替换，Windows 使用 `MoveFileExW` 的替换和写穿标志；失败保留原配置，不在内存假报保存成功。中断可能留下 `.json.tmp`，重启不会盲目覆盖：先检查原文件及临时文件，备份后手动删除残留临时文件。不要同时运行多个实例共享同一配置；不支持外部热编辑。不同文件系统、断电和磁盘故障仍可能影响持久性，不承诺绝对断电安全。

## 资源设计与性能测量

采用 Axum + Tokio 单线程事件循环，HTTP/1，无 WebSocket、轮询、遥测、访问日志或定期磁盘写入。相比手写 HTTP，Axum/Hyper 维护成本和协议健壮性更好；相比线程式同步服务器，空闲连接不独占线程。关闭 Axum 的非必要默认功能。Tokio 文件 I/O 在需要时使用惰性阻塞线程池，因此不承诺整个进程只有一个线程。键盘调用是短同步 FFI，序列一次提交，应用层不加延时；同一进程请求不会交错。忙时返回 429，不堆积旧按键。

配置修改由单独异步锁串行化，磁盘写入期间不占用读取配置的锁；按键不等待磁盘落盘。资源均在编译时嵌入；运行中只保存九按钮配置、最多 16 个会话和当前请求。Mock 会记录全部调用，仅用于开发，不用于长期生产。真实键盘后端不记录历史。网络连接、HTTP 缓冲区、分配器、Tokio 任务、Windows DLL 和工作集页面是预期资源开销来源；大量连接仍会提高占用。

Release 使用 `opt-level=z`、LTO、单 codegen unit、符号剥离和 panic abort，以缩小产物。代价是构建较慢、崩溃诊断能力降低，panic 会退出整个进程。尚未比较 `opt-level=2/3` 的真机内存、延迟差异，不认为体积优化必然降低工作集。

以下 **必须在 Windows 真机 Release 版本上执行**，10–20 MB 是优化方向，不是实测保证。不要用 macOS Mock 数值替代 Windows。进程启动后先等待 30 秒，再记录 60 秒 CPU 时间变化：

```powershell
$p = Start-Process .\mini-deck.exe -ArgumentList '--listen','192.168.1.20:8765' -PassThru
Start-Sleep 30
$p.Refresh()
$cpuStart = $p.TotalProcessorTime.TotalSeconds
$clock = [Diagnostics.Stopwatch]::StartNew()
Start-Sleep 60
$p.Refresh()
[pscustomobject]@{
  PID = $p.Id
  WorkingSetMB = [math]::Round($p.WorkingSet64 / 1MB, 2)
  PrivateMemoryMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)
  CPUSeconds = $p.TotalProcessorTime.TotalSeconds
  OneCoreCPUPercent = 100 * ($p.TotalProcessorTime.TotalSeconds - $cpuStart) / $clock.Elapsed.TotalSeconds
  Threads = $p.Threads.Count
  Handles = $p.HandleCount
}
```

长期监测（独立 PowerShell，`$deckPid` 替换为上面的 PID；每 60 秒采样，连续 24 小时）：

```powershell
$deckPid = 1234
1..1440 | ForEach-Object {
  $q = Get-Process -Id $deckPid -ErrorAction Stop
  [pscustomobject]@{
    Time = (Get-Date).ToString('o')
    WorkingSetMB = $q.WorkingSet64 / 1MB
    PrivateMemoryMB = $q.PrivateMemorySize64 / 1MB
    CPUSeconds = $q.TotalProcessorTime.TotalSeconds
    Threads = $q.Threads.Count
    Handles = $q.HandleCount
  } | Export-Csv .\mini-deck-monitor.csv -Append -NoTypeInformation
  Start-Sleep 60
}
```

监测轮询属于外部测量脚本，服务本身没有轮询。分别测试完全空闲、持续按键、反复修改、手机断网重连，观察私有内存是否持续增长，工作集会受 Windows 回收策略影响。CPU 是累计秒数；相邻采样差除以间隔得到单核占比，任务管理器整机占比还要除以逻辑处理器数。启动用 Stopwatch 测量 Start-Process 到首次健康响应；不要把进程创建耗时当作就绪耗时。HTTP 延迟可用 `curl.exe -w "%{time_total}" -o NUL -s http://127.0.0.1:8765/api/health`，包含客户端及连接开销，需多次测量分布。EXE 大小：`(Get-Item .\mini-deck.exe).Length`。

## 项目结构

```text
mini-deck/
├── Cargo.toml / Cargo.lock
├── src/
│   ├── lib.rs
│   ├── main.rs
│   ├── server.rs
│   ├── config.rs
│   ├── auth.rs
│   ├── shortcut.rs
│   └── keyboard/
│       ├── mod.rs
│       ├── windows.rs
│       └── mock.rs
├── web/
│   ├── index.html
│   ├── style.css
│   └── app.js
├── tests/
│   ├── shortcuts.rs
│   ├── config.rs
│   └── api.rs
├── .cargo/config.toml
├── .github/workflows/windows.yml
├── .gitignore
└── README.md
```

## 验证记录

当前 macOS（x86-64）验证结果：

- `cargo test --locked --offline`：14 个测试通过，0 失败。
- `cargo clippy --all-targets --locked --offline -- -D warnings`：通过。
- `cargo fmt --check`：通过。
- `cargo build --release --locked --offline`：macOS 原生产物构建通过。
- `cargo check --all-targets --locked --offline --target x86_64-pc-windows-msvc`：Windows 目标和测试代码编译检查通过；没有链接或运行 Windows EXE。
- 本机 Release Mock 服务实际 TCP HTTP 冒烟验证通过：页面、健康检查、配对、按钮读取、模拟按键、配置保存、注销及撤销后 401。
- Windows CPU、工作集、私有内存、启动时间、响应延迟、长期稳定性及 EXE 大小：均未实测，不声称达到 10–20 MB。

单元/集成测试涵盖解析、修饰键规范顺序、虚拟键事件、逆序释放及注入失败恢复、无效配置、加载/保存失败、API 鉴权、配对限速/会话上限/注销、Mock 端到端、并发请求和锁释放。测试不会向运行测试的 Windows 电脑注入真实键盘。

Windows 真机仍需验证：实际快捷键及扩展键、UIPI 拒绝行为、部分注入恢复、物理按键干扰、手机浏览器触摸和编辑、Windows 防火墙、配置替换与断电恢复、24 小时稳定性、启动/HTTP 延迟、工作集/私有内存/CPU/最终 EXE 体积。后续优化应以这些测量为依据；必要时增加连接数与空闲连接寿命限制以及 TLS 部署方案。
