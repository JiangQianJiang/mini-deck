Mini Deck 0.3.1 修复 Windows 下载包的目录结构，并移除已无必要的启动脚本。

- 修复 Actions 构建产物中的 EXE 与脚本按源码目录分散的问题。普通构建与正式 Release 使用相同打包流程。
- 下载包根目录统一包含 `mini-deck.exe`、`README.md`、`Measure-MiniDeck.ps1`，不再包含 `启动局域网.cmd`。
- 双击 EXE 即默认开放局域网访问；仅本机访问使用 `mini-deck.exe --local`。
- 打包时验证文件清单，Release 另校验压缩后的 ZIP 目录层级，防止再次分发错误布局。

下载 `mini-deck-windows-x64-v0.3.1.zip` 并解压。升级前退出旧程序，保留 `config.json` 和 `config.json.devices.json`，替换 EXE 后重新启动。旧文件夹中的 `启动局域网.cmd` 可删除。

HTTP 仍未加密，仅在可信私人局域网使用。所有 v0.3.0 功能保持可用。
