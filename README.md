# Mini Deck

轻量 Windows 局域网快捷键控制器。Rust 原生程序提供手机网页，通过 Win32 `SendInput` 向电脑当前前台窗口发送快捷键。无 Node、Python、浏览器运行时、数据库、插件或后台前端服务。

## Windows 下载与升级

仓库：[JiangQianJiang/mini-deck](https://github.com/JiangQianJiang/mini-deck)，目前为私有，需要登录有访问权限的 GitHub 账号。

打开 [Releases](https://github.com/JiangQianJiang/mini-deck/releases)，下载对应版本的 `mini-deck-windows-x64-v版本号.zip`。解压到可写目录，包含：

- `mini-deck.exe`：后台程序，不需要安装 Rust 或其他开发环境。
- `启动局域网.cmd`：双击后监听全部 IPv4 网卡，端口 8765。
- `Measure-MiniDeck.ps1`：手动运行的外部性能测量脚本。
- `README.md`：本文档。

也可仅下载单个 EXE。Release 附带 `SHA256SUMS.txt`，用 PowerShell `Get-FileHash .\mini-deck.exe -Algorithm SHA256` 或对 ZIP 执行同样命令，与其中对应文件名的校验值比较。校验值用于检查传输完整性，不是独立签名。

升级前退出旧程序，将新版文件放回原目录，**保留 `config.json` 和 `config.json.devices.json`**。0.1.x 按钮配置可直接使用。旧版的临时会话不会升级为长期设备。

开发构建也可在 **Actions → Windows 构建和测试 → 成功运行 → Artifacts** 下载 `mini-deck-windows-x64`；这种 ZIP 的文件会按源码目录分层，EXE 位于 `target/release/`。正式版本优先使用 Releases 的平铺安装包。

## 手机连接

1. 手机与电脑连接同一可信私人局域网。
2. 双击解压目录内的 `启动局域网.cmd`；或者在 PowerShell 执行 `mini-deck.exe --lan`。
3. 电脑打开控制台显示的 `http://127.0.0.1:8765/connect`，显示手机连接二维码。
4. 手机扫码打开控制面板，输入电脑控制台的配对码。
5. 在电脑选中目标应用，再在手机点击按钮。

二维码完全在本地生成，只包含 HTTP 地址，不含配对码或会话令牌，不请求外部二维码服务。多网卡时展示多个地址，选择手机能访问的一个；VPN、虚拟网卡、无线客户端隔离和 Windows 防火墙可能影响连接。`--lan` 会监听全部 IPv4 网卡，请仅为私人网络放行程序/端口，不要进行公网端口转发。

仅双击 EXE 默认仍只监听回环地址，适合电脑本机测试。要限制到指定网卡：

```powershell
.\mini-deck.exe --listen 192.168.1.20:8765
```

指定监听地址时，二维码页对应同一个地址下的 `/connect`。指定 IPv6 地址须加方括号。程序在启动时枚举一次网卡，没有网卡轮询；IP 改变后重启更新地址。

配置默认位于 EXE 同目录，可通过 `--config C:\Users\你的名字\mini-deck.json` 修改。Ctrl+C 退出，`--help` 查看参数。非 Windows 只允许显式 `--mock`，不会模拟真实系统按键，网页会显示模拟模式。

## 配对、记住设备和撤销

配对码使用系统随机数生成，32 位随机值，10 分钟有效。五次错误后全局限制配对 60 秒；防止按 IP 绕过，也意味着攻击者可能暂时阻止配对。配对码过期后在电脑重启程序获取新码，已经配对的设备不受影响。

- 默认不勾选“记住此设备”：令牌只驻留当前页面内存，固定 24 小时有效；刷新页面或后台重启后重新配对。
- 主动勾选“记住此设备”：令牌存入当前浏览器的 `localStorage`，固定 30 天有效。刷新和后台重启后自动恢复连接，不是每次请求续期。
- 浏览器禁止本地存储时会显示提示，仍可使用当前页面。隐私模式清理、网站数据删除、IP/端口/协议改变后需重新配对。
- 页面中的“管理已配对设备”列出名称、到期日期、临时/记住状态，可撤销任意设备，包括自己。最多 16 个有效会话。
- “注销设备”同时撤销当前会话并删除浏览器凭据。仅关闭页面不会撤销服务器记录。
- 注销/撤销成功返回后，后续请求会被拒绝；已经开始执行的按键无法撤回。保存失败会明确报错，操作不假报成功，可以重试。

服务端设备文件是配置路径追加 `.devices.json`，如 `config.json.devices.json`。仅持久化已记住会话的 SHA-256 摘要和元数据，不保存原始 Bearer 令牌。原始令牌是系统随机生成的 256 位值。服务端只做摘要匹配，摘要本身不能用作 Bearer 凭据；本地文件也不应分享或手动修改。

如果所有设备丢失或会话已满，**先停止正在运行的 Mini Deck**，执行以下命令清空记住设备，再重启：

```powershell
.\mini-deck.exe --clear-devices
# 使用自定义配置时必须传入同一路径
.\mini-deck.exe --config C:\Users\你的名字\mini-deck.json --clear-devices
```

清空命令只修改磁盘并退出，不会控制另一个正在运行的进程，所以必须先停机。损坏的设备文件会明确报错，不静默重置；上述命令也可用于重置损坏设备文件。

## 安全边界和键盘限制

**HTTP 不加密。配对码、浏览器令牌及按键请求可被同网段攻击者窃听、修改或重放，无法抵御中间人攻击。记住设备是便利功能，不是额外的传输保护。仅限可信私人局域网，勿用于公共 Wi-Fi 或公网。** 只在自己的手机/平板勾选记住设备。第一版没有 TLS、多用户角色隔离或连接级抗 DoS；任何已认证设备都能编辑按钮和撤销其他会话。

API 使用 Bearer 请求头，凭据不放 URL。不开放 CORS，不使用认证 Cookie；JSON 写入、跨站 Fetch 检查、CSP、禁嵌入和禁止缓存头减少第三方页面滥用。HTTP 页面不需要 Service Worker 或 PWA。

快捷键作用于 **电脑当前前台窗口**。普通权限进程不能可靠控制管理员窗口；UIPI 可拒绝注入而不提供有意义的系统错误。锁屏、安全桌面和 Ctrl+Alt+Delete 不支持。必须在已登录的交互式用户会话运行，不能作为 Session 0 系统服务。

修饰键统一按 Ctrl、Alt、Shift、Win 顺序按下，主键随后按下，所有键逆序释放。尽量单次 `SendInput` 提交并检查返回事件数，失败时尽力释放本次组合键，不无条件报告成功。物理按键会影响注入状态；正常执行不预先释放用户按键，失败恢复也可能被系统拒绝。手机请求超时不自动重试，避免重复执行状态未知的快捷键。

## 按钮与配置

默认九宫格：复制、粘贴、剪切、撤销、重做、全选、保存、查找、任务管理器。勾选编辑模式后点击按钮修改名称和快捷键，只有保存成功才更新内存。

支持 Ctrl、Alt、Shift、Win 和一个主键：A–Z、0–9、F1–F12、Enter、Esc、Tab、Space、Left、Right、Up、Down。不会接受 Shell、脚本或多步骤宏。

配置 JSON 顶层 `buttons`，每项 `id/name/shortcut/position`。固定九个按钮，ID 不重复，位置唯一且为 0–8。名称 1–40 字，快捷键最多 80 字节，请求体最多 4 KiB。启动缺少文件时创建默认配置，解析失败报错退出，保留原数据。

修改时在目标目录创建随机且互不冲突的临时文件，写入并同步后原子替换；阻塞文件操作在惰性阻塞任务内完成。使用 `tempfile` 处理跨平台替换与 RAII 清理，普通失败只删除本次创建的文件，不误删其他文件。旧 `.json.tmp` 不再阻塞保存，也不会被盲目当成配置恢复。强制结束进程或断电仍可能遗留新的临时文件，但不会影响下次保存；需要清理时先确认没有实例运行。不同文件系统和断电行为不提供绝对持久性保证。

不要同时运行多个实例共享同一配置或设备文件；不支持外部热编辑。配置与设备记录只在修改时写入，空闲不落盘。

## HTTP API

除静态页面、连接二维码、健康检查和配对外，都需要 `Authorization: Bearer 令牌`。

| 接口 | 输入和结果 |
| --- | --- |
| `GET /api/health` | `ok/platform/mock`，不表示实际按键已经验证 |
| `POST /api/pair` | `{"code":"配对码","remember":true,"name":"我的平板"}` → `token/remembered`；后两项可省略 |
| `GET /api/buttons` | `{"buttons":[...]}` |
| `POST /api/buttons/{id}/press` | 只提交 ID；成功 204，键盘失败 503，忙时 429 |
| `PUT /api/buttons/{id}` | `{"name":"复制","shortcut":"Ctrl+C"}` → 204 |
| `GET /api/devices` | 设备数组：`id/name/expires_at/remembered/current`，不返回令牌或摘要 |
| `DELETE /api/devices/{id}` | 撤销指定会话，保存成功后返回 204 |
| `POST /api/logout` | 撤销当前会话，保存成功后返回 204 |

设备到期时间是 UTC Unix 秒。设备变更单独串行化，落盘时不占用按键鉴权锁或配置读取锁。

## Windows 性能记录

0.2.0 提供外部测量脚本，**尚无目标电脑的真机性能数据**。空闲 CPU 接近 0%、10–20 MB 内存是优化方向，不是保证。CI 只用于功能验证，不用于宣称资源达标。

先运行 Release EXE，另开 PowerShell 获取 PID（有多个实例时自行选择）：

```powershell
Get-Process mini-deck | Select-Object Id, Path
powershell -NoProfile -ExecutionPolicy Bypass -File .\Measure-MiniDeck.ps1 -ProcessId 1234
```

默认预热 30 秒，每 10 秒采样，持续 10 分钟。`ExecutionPolicy Bypass` 仅作用于该脚本进程，不修改系统执行策略。源码目录中的脚本路径是 `scripts\Measure-MiniDeck.ps1`。24 小时记录：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Measure-MiniDeck.ps1 -ProcessId 1234 -DurationSeconds 86400 -IntervalSeconds 60
```

可选进行一次 20 请求的本机健康检查延迟采样（不发送按键）：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\Measure-MiniDeck.ps1 -ProcessId 1234 -DurationSeconds 600 -HealthUrl http://127.0.0.1:8765/api/health
```

如果服务只绑定指定网卡，HealthUrl 也应使用该地址。默认生成独立 `mini-deck-measure-时间/`，输出 `samples.csv` 和 `summary.json`：工作集、私有内存、累计 CPU 秒、区间单核 CPU 占比、线程数、句柄数、峰值和可选延迟中位数/P95。可用 `-OutputDirectory` 指定新目录。脚本监测指定进程，不启动/停止服务；进程退出时尝试结束采样并记录提前停止，PID 复用也会被检测。

分别采集完全空闲、持续按键、反复修改、手机断网重连的记录，观察私有内存/句柄是否持续增长。工作集受 Windows 内存回收影响；单核 CPU 百分比除以逻辑处理器数量，才接近任务管理器的整机占比。延迟包含 PowerShell 客户端、建连、首请求预热和测量开销，不等于纯服务处理时间。测量脚本的采样在外部进程运行，服务没有新增定时轮询。短时 CI 验证测量脚本产生报告，不将其数值当成真机结果。

启动速度应以 Stopwatch 记录 Start-Process 到首次 `/api/health` 成功的时间，不能只测进程创建。EXE 体积：`(Get-Item .\mini-deck.exe).Length`。升级时在相同机器/负载下对比，保存原始数据和测试环境。

## 资源设计与构建

Axum/Hyper + Tokio 单线程事件循环，HTTP/1，静态 HTML/CSS/JavaScript 编译时嵌入。无 WebSocket、持续刷新、遥测、请求日志或后台维护任务。网卡枚举和二维码生成只在启动时进行；二维码页面是预生成字符串，不引入图像库、前端框架或外部服务。

Tokio 文件操作和原子替换使用惰性阻塞线程，因此不承诺进程只有一个线程。配置和设备修改各自串行化，落盘不占用按键读取/鉴权锁。键盘 FFI 是短同步操作，无人为延时，忙时返回 429，不堆积旧请求。会话最多 16 个，只在请求/启动时清理过期记录，无周期清理任务。

主要预期开销来源：网络连接和 HTTP 缓冲区、Tokio 任务、分配器、Windows DLL 和进程页面。新二维码/网卡依赖增加静态代码和启动分配，设备摘要增加少量请求计算，不假设体积变化等于内存变化。`MockKeyboardBackend` 保留全部调用记录，只用于开发和短时测试；生产 Windows 后端不保留历史。

Release 使用 `opt-level=z`、LTO、单 codegen unit、符号剥离、panic abort，减小体积但延长构建且降低崩溃诊断便利性；panic 退出整个进程。未对比 `opt-level=2/3` 的真机内存/延迟，不宣称当前参数最优。Windows x64 MSVC 静态链接 C 运行库，仍依赖 Windows 自带 DLL。

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

macOS `cargo check` 不链接或运行 Windows EXE。GitHub Actions 的 Windows job 执行测试、Release 链接和 Mock HTTP 冒烟验证；`v*` 标签触发专门的发布工作流，验证标签与 Cargo 版本一致，测试通过后发布 ZIP、EXE 和 SHA-256 校验值。上传标签即授权发布对应版本。

## 结构和验证

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
├── .gitignore
├── RELEASE_NOTES.md
└── README.md
```

0.2.0 macOS 本机验证：21 个 Rust 自动化测试通过；`cargo fmt --check`、macOS/Windows 目标 Clippy（`-D warnings`）、Windows 目标全 targets 编译检查、macOS Release 构建及 JavaScript 语法检查通过。本机 Release Mock 的真实 HTTP 冒烟验证了二维码、记住设备跨进程恢复、撤销和重启后仍失效。Windows 实际构建和冒烟结果以对应版本 CI 为准。

0.2.0 本机自动测试覆盖：快捷键解析、规范修饰键顺序、Win32 事件序列、部分注入清理、无效配置、旧临时文件、原子替换失败清理和重试、未认证请求、并发按钮请求、失败后的锁释放、限速/过期/会话上限、记住设备重启恢复、撤销持久化和落盘失败回滚。测试不向 Windows 注入真实按键。Windows 冒烟脚本额外覆盖真实 TCP HTTP、二维码页面和 PowerShell 测量报告生成。

仍需目标 Windows 电脑验证：真实键盘/UIPI/物理按键干扰、手机扫码和浏览器本地存储、睡眠恢复/网络变更、防火墙、磁盘/断电恢复、24 小时稳定性、CPU/工作集/私有内存/延迟。后续以测量结果决定连接数限制、空闲连接超时及编译参数，暂不引入 TLS、托盘、服务框架或自动更新进程。
