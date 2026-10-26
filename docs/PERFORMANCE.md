# Windows 性能测量

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

## 资源设计与开销

Axum/Hyper + Tokio 单线程事件循环，HTTP/1，静态 HTML/CSS/JavaScript 编译时嵌入。无 WebSocket、持续刷新、遥测、请求日志或后台维护任务。网卡枚举和二维码生成只在启动时进行；二维码页面是预生成字符串，不引入图像库、前端框架或外部服务。

Tokio 文件操作和原子替换使用惰性阻塞线程，因此不承诺进程只有一个线程。配置和设备修改各自串行化，落盘不占用按键读取/鉴权锁。键盘 FFI 是短同步操作，无人为延时，忙时返回 429，不堆积旧请求。会话最多 16 个，只在请求/启动时清理过期记录，无周期清理任务。

主要预期开销来源：网络连接和 HTTP 缓冲区、Tokio 任务、分配器、Windows DLL 和进程页面。新二维码/网卡依赖增加静态代码和启动分配，设备摘要增加少量请求计算，不假设体积变化等于内存变化。`MockKeyboardBackend` 保留全部调用记录，只用于开发和短时测试；生产 Windows 后端不保留历史。

Release 使用 `opt-level=z`、LTO、单 codegen unit、符号剥离、panic abort，减小体积但延长构建且降低崩溃诊断便利性；panic 退出整个进程。未对比 `opt-level=2/3` 的真机内存/延迟，不宣称当前参数最优。Windows x64 MSVC 静态链接 C 运行库，仍依赖 Windows 自带 DLL。
