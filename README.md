# Mini Deck

通过手机或平板上的网页，触发 Windows 电脑的键盘快捷键。

Mini Deck 使用 Rust 编写，调用 Win32 `SendInput` 发送按键。Windows 端运行一个 EXE 即可，无需安装 Rust、Node.js 或 Python，也无需单独启动网页服务。

[下载 Windows 版](https://github.com/JiangQianJiang/mini-deck/releases/latest) · [更新记录](https://github.com/JiangQianJiang/mini-deck/blob/main/RELEASE_NOTES.md) · [问题反馈](https://github.com/JiangQianJiang/mini-deck/issues)

## 功能

- 默认提供复制、粘贴、保存等九个快捷键按钮。
- 在网页中修改按钮名称与快捷键，配置自动保存。
- 扫描电脑上显示的二维码连接，通过配对码授权设备。
- 可选择记住设备，并在网页中查看、撤销已配对设备。
- 单进程运行，网页资源内嵌，无持续轮询或外部服务。

## 快速开始

### 1. 下载并解压

运行环境：**Windows 10 / 11，x64**。手机或平板需与电脑连接同一可信私人局域网。

从 [Releases](https://github.com/JiangQianJiang/mini-deck/releases/latest) 下载 `mini-deck-windows-x64-v版本号.zip`，解压到有写入权限的目录。仓库目前为私有，下载需要登录有访问权限的 GitHub 账号。

| 文件 | 用途 |
| --- | --- |
| `mini-deck.exe` | 主程序 |
| `启动局域网.cmd` | 使用局域网模式启动 |
| `Measure-MiniDeck.ps1` | 可选的性能测量脚本 |
| `README.md` | 使用说明 |

### 2. 启动并配对

1. 双击 `启动局域网.cmd`，保持控制台窗口打开。
2. 在电脑浏览器打开 `http://127.0.0.1:8765/connect`，显示连接二维码。
3. 手机扫码进入控制面板，输入电脑控制台中的配对码。
4. 在电脑上切换到目标应用，然后在手机上点击快捷键按钮。

也可以在手机浏览器直接输入控制台显示的局域网访问地址。Windows 防火墙询问时，仅允许可信的**私人网络**访问。

> **安全提示：** 当前使用 HTTP，连接不加密，配对不能防止局域网窃听或中间人攻击。仅在可信私人局域网使用，不要开放公网端口。

### 3. 编辑按钮

在控制面板启用编辑模式，点击按钮修改名称和快捷键，保存后生效。

支持 `Ctrl`、`Alt`、`Shift`、`Win` 与一个主键组合：`A–Z`、`0–9`、`F1–F12`、`Enter`、`Esc`、`Tab`、`Space`、`Left`、`Right`、`Up`、`Down`。例如 `Ctrl+C`、`Ctrl+Shift+Esc`。

### 4. 退出与升级

在控制台按 `Ctrl+C` 退出。

升级前退出旧程序，将新版文件解压到原目录，保留 `config.json` 和 `config.json.devices.json`。0.1.x 的按钮配置可继续使用，但临时会话需要重新配对。

Release 附带 `SHA256SUMS.txt`。可执行以下命令，将结果与清单中的对应文件比较，检查下载完整性：

```powershell
Get-FileHash .\mini-deck.exe -Algorithm SHA256
```

## 配对与设备管理

配对码启动时生成，10 分钟内有效；过期后重启程序获取新码。连续五次错误后暂停配对 60 秒。

| 配对方式 | 有效期 | 刷新页面或重启程序后 |
| --- | --- | --- |
| 默认临时配对 | 24 小时 | 需要重新配对 |
| 勾选“记住此设备” | 30 天 | 自动恢复，浏览器需保留网站数据 |

页面中的“管理已配对设备”可撤销设备，“注销设备”可撤销当前会话。最多保留 16 个有效会话。请只在自己的设备上选择记住设备；所有已配对设备都有编辑按钮和撤销其他设备的权限。

## 启动参数与配置

直接双击 `mini-deck.exe` 默认只允许本机访问。手机连接时使用启动脚本，或在 PowerShell 中运行：

```powershell
.\mini-deck.exe --lan
```

| 参数 | 说明 |
| --- | --- |
| `--lan` | 监听所有 IPv4 网卡，端口为 `8765` |
| `--listen IP:端口` | 指定监听地址；默认 `127.0.0.1:8765` |
| `--config 文件路径` | 指定配置文件；默认位于 EXE 同目录的 `config.json` |
| `--clear-devices` | 清空已记住的设备后退出，须先停止旧实例 |
| `--mock` | 模拟模式，不发送真实按键 |
| `--help` | 显示帮助 |

例如，只监听指定网卡：

```powershell
.\mini-deck.exe --listen 192.168.1.20:8765
```

首次启动会创建默认配置。按钮修改时才写入磁盘；设备记录保存在配置路径追加 `.devices.json` 的文件中。不要让多个实例共用同一配置文件，也不要在程序运行时手动编辑配置。

## 常见问题

### 手机打不开页面

确认使用局域网模式启动，手机与电脑在同一网络，且防火墙允许私人网络访问。多网卡时尝试控制台列出的其他地址；VPN、访客网络和无线客户端隔离可能阻止连接。电脑 IP 改变后，重启程序并重新扫码。

### 页面显示成功，目标应用却没有响应

快捷键发送到电脑的当前前台窗口，请先选中目标应用。普通权限程序无法可靠控制管理员权限窗口。锁屏、安全桌面和 `Ctrl+Alt+Delete` 不支持。

程序必须运行在已登录的交互式 Windows 会话中，不能作为 Session 0 系统服务运行。手机请求超时后，请确认电脑上的执行结果，再决定是否重试。

### 无法配对或设备名额已满

配对码过期时重启程序；达到尝试限制时等待 60 秒。如果无法通过现有设备撤销会话，先退出正在运行的 Mini Deck，再执行：

```powershell
.\mini-deck.exe --clear-devices
```

使用自定义配置时，须同时传入相同的 `--config` 路径。随后重新启动并配对。

### 配置保存失败

检查目录写入权限、磁盘空间以及文件是否被其他程序占用，排除问题后重试。保存失败会报错，原配置不会被当作新配置覆盖。配置文件损坏时程序会报错退出，请备份后检查文件内容。

## 开发与技术文档

技术栈：Rust、Axum、Tokio、Win32 `SendInput`，前端使用原生 HTML、CSS 和 JavaScript。

- [开发、构建与测试](https://github.com/JiangQianJiang/mini-deck/blob/main/docs/DEVELOPMENT.md)
- [HTTP API](https://github.com/JiangQianJiang/mini-deck/blob/main/docs/API.md)
- [配对机制与安全边界](https://github.com/JiangQianJiang/mini-deck/blob/main/docs/SECURITY.md)
- [Windows 性能测量](https://github.com/JiangQianJiang/mini-deck/blob/main/docs/PERFORMANCE.md)

Windows 开发机安装 Rust MSVC 工具链与 Visual Studio C++ Build Tools 后，可执行：

```powershell
cargo test --locked
cargo build --release --locked
.\target\release\mini-deck.exe --lan
```

macOS 可通过 Mock 后端开发和测试，不能验证真实 Windows 按键注入。Windows CI 执行构建、自动化测试与 Mock HTTP 冒烟测试；实际按键效果和资源占用仍需 Windows 真机验证。

项目优先考虑低空闲资源占用，但目前没有目标电脑的完整性能测量结果。空闲 CPU 接近 0%、内存 10–20 MB 为优化目标，不是已验证保证。性能脚本及长期监测方法见上述测量文档。
