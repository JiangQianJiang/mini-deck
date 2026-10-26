# HTTP API

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
