# HTTP API

## 认证与约定

除静态页面、连接二维码、健康检查和配对外，接口均要求请求头：

```http
Authorization: Bearer <token>
```

凭据不得放入 URL。写入接口使用 `Content-Type: application/json`，请求体上限为 4 KiB。业务错误通常返回 `{"error":"错误说明"}`；JSON 格式或字段类型不正确时由 Axum 返回 400/422，缺少 JSON Content-Type 时为 415。

| 接口 | 请求及结果 |
| --- | --- |
| `GET /api/health` | 返回 `ok/platform/mock`；不代表实际按键已验证 |
| `POST /api/pair` | `{"code":"配对码","remember":true,"name":"我的平板"}` → `token/remembered`；后两项可省略 |
| `GET /api/buttons` | 返回 `{"buttons":[...]}`，包含各按钮的 `revision` |
| `POST /api/buttons/{id}/press` | `{"revision":0}` → 204 |
| `PUT /api/buttons/{id}` | `{"name":"复制","shortcut":"Ctrl+C","revision":0}` → 204 |
| `GET /api/events` | 经过 Bearer 认证的 SSE 配置变化通知 |
| `GET /api/devices` | 返回设备数组：`id/name/expires_at/remembered/current`，不含令牌或摘要 |
| `DELETE /api/devices/{id}` | 撤销指定会话 → 204 |
| `POST /api/logout` | 撤销当前会话 → 204 |

设备到期时间为 UTC Unix 秒。临时会话撤销只更新内存；已记住设备在持久化成功后才返回撤销成功，写盘失败返回 500 并保留原会话。认证变更串行处理。

## 支持的按键

主键支持 A–Z、0–9、F1–F12、Enter、Esc、Tab、Space、方向键 Left/Right/Up/Down，以及 PageUp、PageDown、Home、End、Insert、Delete、Backspace，可与 Ctrl/Alt/Shift/Win 组合。名称不区分大小写；PgUp、PgDn/PgDown、Ins、Del 分别为对应导航/编辑键的别名。网页手动选择和录制支持相同的主键范围。

Win32 虚拟键码依据 [Microsoft 虚拟键码表](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)。PageUp/PageDown、Home/End、Insert/Delete 使用独立导航键的扩展标记，Backspace 不设置该标记。

## 按钮版本与冲突

按钮示例：

```json
{
  "id": 0,
  "name": "复制",
  "shortcut": "Ctrl+C",
  "position": 0,
  "revision": 0
}
```

`revision` 是每个按钮独立的非负 32 位整数。旧配置缺少该字段时按 `0` 加载，不在启动时重写旧文件。成功编辑仅递增该按钮的版本；保存失败不修改内存、不递增版本、不发出通知。版本达到上限时拒绝进一步编辑，不回绕。

触发请求必须提交页面显示的按钮版本，不能只提交 ID。服务端在同一次配置加锁读取中检查版本并取得快捷键快照。编辑请求在配置更新锁内检查版本，持久化成功后发布内存状态与通知。接收后的编辑事务不会因 HTTP 请求断开而停在“磁盘已更新、内存未发布”的中间状态。

| 状态码 | 含义及处理 |
| --- | --- |
| 204 | 本次操作成功；编辑后重新获取配置 |
| 401 | 会话失效；停止同步并重新配对 |
| 409 | 按钮版本冲突，未执行按键或覆盖配置；获取最新配置并由用户再次确认 |
| 429 | 键盘正忙、配对限流或 SSE 连接已满，错误信息说明具体原因 |
| 500 | 配置或设备持久化失败等内部错误 |
| 503 | 按键注入失败，可能有部分事件已提交；不要自动重试 |

缺少 `revision` 的旧触发/编辑请求会被拒绝。升级后应刷新所有已打开的页面。按键请求发生冲突、网络错误或超时，客户端均不得自动重发。

## SSE 同步

使用 `fetch('/api/events', {headers: {Authorization: 'Bearer …'}, signal})` 读取事件流，不使用无法设置 Bearer 请求头的原生 `EventSource`，不把凭据放入查询参数。

成功连接时 Content-Type 为 `text/event-stream`，立即发送一次事件；配置修改成功后再次通知：

```text
event: config
data: {}

```

事件只表示“需要重新读取完整配置”，不包含配置、凭据或设备信息，也不是可回放的事件日志。不使用 Last-Event-ID 恢复。初次连接、每次重连及页面重新可见时，客户端均须调用 `GET /api/buttons` 获取完整配置。

服务端使用 `watch` 合并变化信号，每条流仅保留最新信号，不为慢客户端创建无界队列。最多同时接受 32 条 SSE 流；连接关闭后释放名额。会话到期或撤销后终止流，重连返回 401；服务退出时也会主动结束流，避免阻塞优雅退出。没有周期心跳；连接异常由网络错误、流结束、页面重新可见或手动重连触发恢复。

内置页面使用流式 UTF-8 解码，支持事件跨分片、CR/LF 换行和多行 data，并限制单个事件大小。重复通知合并刷新；用请求序号和会话代次拒绝过期响应。隐藏页面时取消流，重新可见时重新读取配置并连接。流建立超时为 8 秒，断开后最多重试 5 次，间隔为 1、2、4、8、16 秒；达到上限后显示手动重连入口。

## 配对码刷新

没有 HTTP 刷新接口。电脑端在运行中的交互式控制台输入 `r` 并回车，生成新码并只在该控制台显示。旧码立即失效，有效期重新计算为 10 分钟，已有会话与暴力尝试限制保持不变。
