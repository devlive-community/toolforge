# ToolForge 插件下载统计服务

Go 单二进制服务，使用 MySQL 8.0 / InnoDB。COS 负责文件流量，服务负责下载 302、客户端验证后的成功回报和下载量查询。不使用 COS 访问日志，不记录回报凭据，不提供公网管理写入接口。

## 统计口径

“下载量”是客户端完成文件下载，并校验包大小、SHA-256、官方签名以及插件 ID/版本后确认的下载次数。它不是安装成功量、独立用户数或浏览器点击数。公开客户端可以被修改，服务不能绝对证明真实下载或彻底防刷。

同一次逻辑下载使用同一个 128 位随机 `attempt_id`。发起下载和成功回报分别采用唯一键和 MySQL 事务幂等。不同次的实际下载可以分别计数。旧客户端可以自动跟随 302，但不能回报成功，因此仅增加发起次数。

服务正常回报期限为 7 天。客户端回报队列保留不超过 5 天；确认记录至少保留 30 天。过期尝试可删除，永久累计计数不随清理变化。用户重新操作时必须生成新的随机 ID；不得跨逻辑下载重用 ID。

## 配置与运行

最低 Go 1.26.10，后续使用受支持系列的最新补丁版本。该最低版本包含 [GO-2026-6809](https://pkg.go.dev/vuln/GO-2026-6809) 的 crypto/x509 修复；如改用 Go 1.27 系列，至少使用 1.27.3。间接依赖 edwards25519 固定到 1.1.1 以修复 [GO-2026-4503](https://pkg.go.dev/vuln/GO-2026-4503)。运行服务器不需要 Go 工具链。依赖由 `go.mod` / `go.sum` 固定。

在本目录执行：

```sh
go test ./...
go test -race ./...
go vet ./...
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 go build -trimpath -ldflags "-s -w -X main.buildSHA=<commit-sha>" -o plugin-stats ./cmd/plugin-stats
```

命令：

```text
plugin-stats version
plugin-stats migrate up
plugin-stats catalog import <release-catalog.json>
plugin-stats serve
```

不传参数等同 `serve`。`version` 输出构建提交。CLI 错误不打印 DSN、密码或回报密钥；数据库底层错误不透传到 HTTP。

配置见 `.env.example`。生产通过 systemd 的受限 `EnvironmentFile` 提供。`migrate` 和 `catalog` 命令只需要数据库设置，不读取或需要回报密钥。为常驻服务、迁移及目录导入使用三个不同 MySQL 用户，数据库独立于服务器现有业务数据库。

| 配置 | 说明 |
|---|---|
| `MYSQL_USER` / `MYSQL_PASSWORD` | 专用账号，不接受 root |
| `MYSQL_DATABASE` | 默认 `toolforge_stats` |
| `MYSQL_SOCKET` | 默认 `/var/run/mysqld/mysqld.sock` |
| `MYSQL_DSN` | 可选，覆盖分项；只允许本机 socket 或 IP loopback TCP |
| `STATS_LISTEN_ADDR` | 默认 `127.0.0.1:8088`，只允许 loopback |
| `PLUGIN_ASSET_BASE_URL` | 默认 `https://plugins.toolforge.devlive.org`，必须为 HTTPS origin |
| `RECEIPT_ACTIVE_KID` | 新下载使用的密钥编号 |
| `RECEIPT_KEYS_JSON` | 编号到标准 base64 密钥的 JSON 字典，每个密钥至少 32 随机字节，最多 8 个 |

密钥仅存于服务器。轮换时先部署同时包含旧、新密钥的 keyring，再切换 active kid；旧密钥至少保留到所有旧凭据过期（7 天加时钟余量）。不得提前删除旧签名密钥。不得在日志、查询字符串、COS 请求或客户端持久化配置中暴露服务端密钥。

建议数据库授权（账号和密码由管理员单独创建，不写入仓库）：

- runtime：`plugin_versions`、`catalog_state`、`schema_migrations` 的 SELECT；`download_attempts` 的 SELECT/INSERT/UPDATE/DELETE；`plugin_stats`、`plugin_version_stats` 的 SELECT/UPDATE。
- catalog：`plugin_versions` 的 SELECT/INSERT/UPDATE；`plugin_stats`、`plugin_version_stats` 的 SELECT/INSERT；`catalog_state` 的 SELECT/UPDATE。
- migrate：仅该数据库所需的 CREATE/ALTER/DROP/INDEX/REFERENCES/SELECT/INSERT/UPDATE/DELETE；不授予跨库权限、FILE、GRANT OPTION 或服务器管理权限。

迁移库使用 MySQL advisory lock 防止并发迁移。DDL 有隐式提交，失败时迁移可能处于 dirty；停止发布并由管理员核验数据库及备份后修复，不自动 force 或执行 down。down SQL 仅用于隔离测试数据库。

## HTTP 接口

所有接口由 Nginx 经 HTTPS 暴露。生产 Nginx 应为新下载限流、设置 body 限制，只信任本机代理，并屏蔽 `/readyz` 的公网访问（部署在服务器本机检查）。服务仅监听 loopback；自身配置 8 秒请求上下文、明确连接超时、4096 字节成功回报 body、128 个并发请求上限和参数限制。无需浏览器 CORS，桌面 Rust 发起请求。

### 发起下载

```text
GET /v1/plugins/{id}/versions/{version}/download
Idempotency-Key: <32 个小写十六进制字符>
```

返回 `302`：

```text
Location: https://plugins.toolforge.devlive.org/packages/<id>/<version>/<sha256>.tfp
X-ToolForge-Attempt-Id: <attempt-id>
X-ToolForge-Receipt: <signed-bearer-receipt>
X-ToolForge-Receipt-Expires: <Unix 秒>
Cache-Control: no-store
```

客户端必须关闭首跳的自动跳转，读取凭据后单独请求 COS，并严格校验预期主机和 HTTPS。不要把回报凭据转发给 COS。新客户端遇到统计入口故障时显示失败并允许重试，不绕过入口直连 COS。

相同 key 和相同版本返回同一套固定元数据、签发时间、到期时间及密钥编号；重试不延长寿命。key 对应其他插件/版本返回 `409`。未知版本 `404`；withdrawn 拒绝新下载，但已有尝试仍可重取有效凭据；revoked 拒绝全部下载和回报。缺少 key 时为兼容旧客户端由服务随机创建 ID。非空格式错误 key 返回 `400`；HEAD 不创建下载。

### 成功回报

```text
POST /v1/downloads/{attempt-id}/complete
Authorization: Bearer <receipt>
Content-Type: application/json

{"sha256":"<64 个小写十六进制字符>","size":12345}
```

返回 `200 {"attemptId":"...","status":"confirmed"}`。同一次回报重复提交同样返回 200，但版本及插件总数都只增加一次。单事务锁定尝试、更新状态及两张计数表。

- `401`：坏凭据；`404`：未知尝试；`409`：元数据或 attempt 不匹配；`410`：过期或安全撤销。
- 请求过大或结构错误：`400`；错误 Content-Type：`415`。
- 数据库/服务临时错误：`503`。客户端仅对网络错误、429、5xx 做有限退避重试并遵守 `Retry-After`，永久错误终止队列项目。

### 统计查询

下载计数是十进制 **JSON 字符串**，避免 JavaScript 数字精度损失。时间是 UTC RFC3339。所有查询采用一致性快照，返回 `countedSince` 与 `asOf`；公开查询可缓存 30 秒。

| 接口 | 结果 |
|---|---|
| `GET /v1/plugins/{id}/versions/{version}/stats` | `{pluginId,version,downloads,totalDownloads,countedSince,asOf}` |
| `GET /v1/plugins/{id}/stats?limit=50&offset=0` | `{pluginId,totalDownloads,versions:[{version,downloads}],limit,offset,hasMore,countedSince,asOf}` |
| `GET /v1/stats?ids=id1,id2` | `{plugins:[{pluginId,totalDownloads,versions:[{version,downloads}],versionsTruncated?}],countedSince,asOf}` |
| `GET /v1/stats/total` | `{totalDownloads,countedSince,asOf}` |
| `GET /healthz` | `{status:"ok",buildSha}` |
| `GET /readyz` | `{status:"ready",buildSha,catalogReady}`，schema/数据库失败时 503 |

批量最多 100 个 ID，每插件最多最近 100 个版本（以发布时间、版本字符串稳定排序），更多时 `versionsTruncated:true`，可按插件详情继续分页。详情 limit 最大 100、offset 最大 100000。未知 ID 在批量结果中省略；单独查询返回 404。未发生下载的已发布版本为 `"0"`；临时查询失败不得在界面伪装为 0。

首次迁移后、目录尚未导入时，readyz 可以返回 `catalogReady:false`，允许先部署服务再完成插件发布。部署必须比较返回的 buildSha 与目标 SHA；目录导入成功后应验证 catalogReady 为 true。

## 全版本目录

本地管理命令导入完整目录：

```json
{
  "schemaVersion": 1,
  "sourceCommit": "0000000000000000000000000000000000000000",
  "generation": 1,
  "plugins": [
    {
      "id": "example-plugin",
      "version": "1.0.0",
      "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "size": 42,
      "objectKey": "packages/example-plugin/1.0.0/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.tfp",
      "status": "active"
    }
  ]
}
```

`generation` 必须为正、单调增加的发布序列，例如同一 Plugins workflow 的 `GITHUB_RUN_NUMBER`。手动引导也必须使用同一发布序列，不能任意设置较大数值。`sourceCommit` 为 40 或 64 个小写十六进制字符。`publishedAt` 可选，省略时第一次导入时间固定为发布时间，重复导入不刷新。

同一 generation + 相同规范 JSON 摘要可幂等重跑；相同 generation 不同内容或较旧 generation 拒绝。包 size 最大 64 MiB，与桌面安装器一致。最多 10000 条、16 MiB 目录；插件 ID/版本和对象路径严格校验，对象路径必须按包 SHA 固定。已登记版本的 SHA、大小、objectKey 不可修改，必须升版。

完整目录中缺失的旧 active 版本自动 withdrawn，保留统计并接受已签发凭据。明确 revoked 的版本具有粘性，后续目录不能恢复为 active/withdrawn；安全撤销后恢复需新版本。管理命令不向数据库导入任意外部下载 URL。

发布顺序必须是：上传并读回验证所有不可变 COS 包 → 保存完整目录/索引快照 → 导入数据库目录 → 更新公共 registry。跨 COS/MySQL 无事务；任一步失败都保留既有公共索引，通过相同摘要对账重跑，不能用删除历史包来“回滚”。

## 数据、备份与恢复

- `plugin_versions`：全版本官方制品目录和 active/withdrawn/revoked 状态。
- `download_attempts`：固定的签发元数据、过期时间、确认状态及幂等键。
- `plugin_version_stats`、`plugin_stats`：永久累计计数。
- `catalog_state`：全目录摘要、代次、来源提交、首次计数时间。
- `schema_migrations`：迁移版本和 dirty 标记。

每小时删除一批（最多 1000 条）过期尝试；已确认的记录还必须确认超过 30 天。高负载时应观察清理积压，再按负载调整频率；不删除永久计数和版本历史。

定期备份全部统计数据库，并实际演练恢复。累计计数与幂等尝试状态必须从同一一致性备份一起恢复，不能单独还原其中一张表。尝试明细清理后，永久总量不能由短期明细完整重建。快照恢复会损失备份时间点之后的数据；不要宣称无损回滚。

服务回滚通过前一版本二进制加兼容 schema 完成。公共索引回滚到已保存快照；不删除已发布包、不执行生产 down、不重新签名历史同版本包。HMAC keyring 不随程序版本回退到缺少现存验证密钥的配置。

## 集成测试

普通 `go test` 会执行 HTTP/配置/目录/签名测试；真实 MySQL 测试仅在明确设置 `TEST_MYSQL_DSN` 时运行，未配置会报告 skip。

```sh
TEST_MYSQL_DSN='<isolated-user>:<password>@tcp(127.0.0.1:3306)/toolforge_stats_test?parseTime=true' go test -race ./...
```

测试会删除并重建已知表，数据库名必须以 `_test` 结尾。绝不能指向开发业务库或生产数据库。CI 应使用隔离 MySQL 8.0 服务；较新本机 MySQL 通过测试仅作为补充，不能替代准确版本兼容验证。

真实测试覆盖：并发相同请求只创建一次尝试、100 次并发确认仅计一次、计数表故障导致事务全回滚、不可变版本冲突、旧目录拒绝、普通下架、安全撤销不可复活、过期回报、明细清理、查询精度与分页。HTTP 单测覆盖旧版跳转、首跳凭据、篡改与过期、参数限制和错误脱敏。
