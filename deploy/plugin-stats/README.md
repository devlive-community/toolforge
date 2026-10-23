# ToolForge 插件统计服务部署

本文和目录内文件是部署模板；提交代码或运行本地检查不会自动初始化服务器、修改现有 Nginx 站点、读取服务器凭据或部署。首次上线由管理员核对后执行。

## 快速开始（推 dev 自动部署）

服务器只需要 MySQL 和一个能用 sudo 的 SSH 账号（root 也行）；服务安装、迁移、重启全部由 workflow 远程完成。

1. 服务器建库建账号（执行一次，密码建议只用字母数字，避免 `@` 和引号）：

```sql
CREATE DATABASE toolforge_stats CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER 'toolforge_stats'@'localhost' IDENTIFIED BY '换成你自己的密码';
GRANT ALL PRIVILEGES ON toolforge_stats.* TO 'toolforge_stats'@'localhost';
FLUSH PRIVILEGES;
```

2. 生成回报签名密钥：`openssl rand -base64 32`。
3. 仓库 Settings → Secrets and variables → Actions 填写：

| 类型 | 名称 | 值 |
| --- | --- | --- |
| secret | `SSH_HOST` | `43.162.108.186` |
| secret | `SSH_USER` | 服务器 SSH 账号（root 或有 sudo 的账号） |
| secret | `SSH_KEY` | 对应私钥全文 |
| secret | `DB_HOST` | `127.0.0.1`（服务必须连本机回环） |
| secret | `DB_PORT` | `3306` |
| secret | `DB_USER` | `toolforge_stats` |
| secret | `DB_PASSWORD` | 上面建账号时设置的密码 |
| secret | `DB_NAME` | `toolforge_stats` |
| secret | `RECEIPT_KEY` | 第 2 步生成的 base64 密钥 |
| secret | `COS_SECRET_ID` / `COS_SECRET_KEY` | 腾讯云密钥；桶为**私有读写**时必需——服务用它给 302 目标生成 10 分钟有效的预签名下载 URL，与 Plugins 发布用同一组即可（不填则要求桶公开读） |

部署脚本不会改动 Nginx 与 `stats.toolforge.devlive.org` 的任何配置（HTTPS、证书、站点文件都归你自己管理）；它只安装二进制、写入 `/etc/toolforge-plugin-stats/runtime.env` 并重启 `toolforge-plugin-stats` 服务。

4. 推送改动 `apps/server/**` 的提交到 `dev` → 自动测试、构建、上传、迁移、重启、健康检查，失败自动回滚旧版本；`workflow_dispatch` 可手动补部署。

需要收紧安全（专用受限 SSH 登录、独立数据库角色）时再参考下文加固，不是必需步骤。

## 架构与顺序

- `plugins.toolforge.devlive.org` 保持腾讯云 COS 内容域名；桶为 `toolforge-1258585472`，地域先按 `ap-singapore` 配置，上线前确认。
- `stats.toolforge.devlive.org` 新增 A 记录至 `43.162.108.186`，由该机现有 Nginx 提供 TLS，反代 `127.0.0.1:8088`。
- App 插件索引中的下载 `url` 字段统一为统计 API；首跳 302 后才下载 COS，不提供统计故障时的 COS 直连降级。
- 顺序：数据库及账号 → 管理员安装受限入口/systemd → 部署服务 → 设置新站点和 TLS → Plugins workflow 首次 bootstrap → 新 App。
- `/readyz` 用于本机部署，确认 schema 和运行提交 SHA。首次还没有目录时 `catalogReady:false`，服务可以启动；目录及索引上线后再检查 `catalogReady:true`。

## 首次服务器准备

先检查已有进程、端口、数据库备份和 Nginx include 结构；不得覆盖现有站点或改 MySQL 监听。既有 MySQL TCP 端口不是默认 3306，本服务优先使用 `/var/run/mysqld/mysqld.sock`。服务器不需要安装 Go，基础工具需要 `bash`, `curl`, `jq`, `flock`, `sha256sum`, `runuser`。

建议创建三个 `--system --no-create-home --shell /usr/sbin/nologin` OS 用户及对应同名组：

| OS 用户 | 用途 |
| --- | --- |
| `toolforge-stats` | 常驻 HTTP 服务 |
| `toolforge-stats-migrate` | 独立数据库迁移进程 |
| `toolforge-stats-catalog` | 可信全版本目录导入 |

管理员安装：

- `deploy.sh` → `/usr/local/sbin/toolforge-plugin-stats-deploy`，`root:root 0755`。
- `toolforge-plugin-stats.service` → `/etc/systemd/system/toolforge-plugin-stats.service`，`root:root 0644`。
- `/opt/toolforge-plugin-stats/releases/` 和 `/opt/toolforge-plugin-stats/current` 只允许管理员写；部署入口维护它们。
- `/etc/toolforge-plugin-stats/`：`root:toolforge-stats 0750`。
- `runtime.env`：`root:toolforge-stats 0640`；`migrate.env`、`catalog.env`：`root:root 0600`。
- 执行 `systemctl daemon-reload` 和 `systemctl enable toolforge-plugin-stats`。首次二进制到达前不要启动空服务。

`migrate.env`、`catalog.env` 仅由管理员入口读取，再以干净环境执行相应低权限用户；无需让这些用户读取配置文件。

### 数据库及配置

新建 `toolforge_stats` 数据库（InnoDB、utf8mb4）；不要复用已有业务库、root 或已有应用账号。凭据通过管理员的安全配置流程产生，不写入 Git、不贴到 issue/chat。密码如含 shell 特殊字符需使用正确引用。配置格式使用 systemd 和 shell 都支持的单行 `KEY=value`/`KEY='value'`，不使用命令替换、变量展开或可执行脚本。

每份文件使用各自的 MySQL 用户：

```dotenv
MYSQL_SOCKET=/var/run/mysqld/mysqld.sock
MYSQL_DATABASE=toolforge_stats
MYSQL_USER=<role-specific-user>
MYSQL_PASSWORD='<set-securely-on-server>'
```

`runtime.env` 另外配置：

```dotenv
STATS_LISTEN_ADDR=127.0.0.1:8088
PLUGIN_ASSET_BASE_URL=https://plugins.toolforge.devlive.org
RECEIPT_ACTIVE_KID=2026-01
RECEIPT_KEYS_JSON='{"2026-01":"<base64-of-at-least-32-random-bytes>"}'
# 可选：看板插件名来源；未设置时使用公共插件索引，显式置空则看板只显示插件 ID
PLUGIN_REGISTRY_URL=https://plugins.toolforge.devlive.org/registry.json
```

服务进程内嵌公共统计看板（`/` 页面与 `/assets/dashboard.js|css`），数据来自 `/v1/dashboard` 聚合接口；Nginx 站点需放行 `location = /` 与 `location /assets/` 两个前缀（见模板），其余路径仍返回 404。

不要把占位符直接投入生产。HMAC 密钥只属于服务运行配置，迁移和目录导入无需此密钥；轮换保留旧密钥至所有旧凭据过期。MySQL 用户为本机 `localhost` 连接授权，先以对应 OS 身份测试 socket 权限。

权限建议（以实际迁移及服务文档为准，全部仅限新数据库）：

| MySQL 角色 | 所需权限 |
| --- | --- |
| runtime | `plugin_versions` 查询；`download_attempts` 查询/插入/更新/清理；`plugin_version_stats`、`plugin_stats` 查询/插入/更新；`catalog_state`、`schema_migrations` 查询 |
| catalog | `plugin_versions`、`plugin_version_stats`、`plugin_stats`、`catalog_state` 查询/插入/更新；`schema_migrations` 查询 |
| migrate | 本库内建表、改表、索引及迁移状态读写；不授予其他库或全局权限 |

上线前启用现有备份或为本库配置备份，确认恢复演练。计数表和幂等记录需恢复到同一备份时间点；不能只恢复一方。迁移采用兼容旧程序的新增方式，生产不自动执行 `down`。

### SSH 身份隔离

不要将日常 `ubuntu` 通用 SSH 私钥放入 Actions。建议创建专用部署与目录导入登录账号，密码锁定，SSH key 仅允许受限 forced command。两个账号使用两把不同的 key，禁止 PTY、端口/代理/X11 转发及用户 rc（OpenSSH `restrict`）。

部署 key 对应 `authorized_keys` 的 forced command：

```text
restrict,command="sudo -n /usr/local/sbin/toolforge-plugin-stats-deploy ssh deploy \"$SSH_ORIGINAL_COMMAND\"" <deploy-public-key>
```

目录 key：

```text
restrict,command="sudo -n /usr/local/sbin/toolforge-plugin-stats-deploy ssh catalog \"$SSH_ORIGINAL_COMMAND\"" <catalog-public-key>
```

sudoers 只允许各自入口，使用 `visudo -cf` 验证后安装（账号名称按管理员实际选择替换）：

```sudoers
<deploy-login> ALL=(root) NOPASSWD: /usr/local/sbin/toolforge-plugin-stats-deploy ssh deploy *
<catalog-login> ALL=(root) NOPASSWD: /usr/local/sbin/toolforge-plugin-stats-deploy ssh catalog *
```

入口不使用 `eval`，拒绝任意路径和其他命令，校验提交 SHA、内容摘要和流大小；catalog 身份无法上传/运行新二进制。服务器文件由管理员持有，Actions 不更新入口、sudoers、Nginx、systemd 或环境配置。

从已经信任的管理连接核对服务器主机公钥，将完整 known_hosts 内容配置到 GitHub Environment；不要在流水线中以未验证的 `ssh-keyscan` 替代验证。

## Nginx 与 TLS

先确认 DNS、生效的站点目录与证书管理方式。新增独立站点 `nginx.conf`，保留其他站点；模板里的 `limit_req_zone`/`log_format` 必须位于 `http` 上下文。首次证书不存在时只启用 HTTP/ACME 段，取得证书后再启用 HTTPS 段。适配已有 ACME 自动续期和实际证书路径。

执行 `nginx -t` 成功后 reload。API 进程只监听回环，`/healthz`、`/readyz` 不公开；代理覆盖来源头，服务只信任本机代理。模板限制请求体为 4 KiB，分别限制下载、回报、查询请求；不缓存 302 和回报，不写 IP、查询串、请求体、Authorization 或响应凭据到访问日志。服务器异常日志保留必要诊断。**不启用 COS 访问日志。**

## GitHub 配置

部署 workflow 直接读取仓库 Actions secrets（见快速开始表格）；数据库密码与回报 HMAC 密钥也由 secrets 注入并在每次部署时写入服务器 `/etc/toolforge-plugin-stats/runtime.env`（`root:toolforge-stats 0640`）。使用受保护 Environment 或仓库级 secret 均可，workflow 未绑定 Environment。

| 类型 | 名称 | 用途 |
| --- | --- | --- |
| secret | `TOOLFORGE_PLUGIN_KEY` | 既有官方插件签名种子；只给独立签名步骤（Plugins 发布用） |
| secret | `COS_SECRET_ID` / `COS_SECRET_KEY` | 腾讯云 COS 上传身份，限制桶及必要对象前缀，优先临时凭据（Plugins 发布用） |
| secret | `COS_SESSION_TOKEN` | 使用临时凭据时配置，长期受限凭据可留空 |
| secret | `PLUGIN_CATALOG_SSH_KEY` | 只给插件发布的受限导入 key |
| variable | `COS_REGION` | 默认 `ap-singapore`，上线前核实 |
| variable | `PLUGIN_CATALOG_SSH_TARGET` | `<catalog-login>@43.162.108.186` |

统计服务部署所需 secrets 见「快速开始」表格（`SSH_*`、`DB_*`、`RECEIPT_KEY`）。上表 COS 与目录导入凭据属于 Plugins 发布 workflow；若沿用旧的受限部署入口（`deploy.sh`），再补充 `STATS_DEPLOY_SSH_KEY`、`STATS_DEPLOY_SSH_TARGET`、`PLUGIN_SSH_KNOWN_HOSTS` 三项。

数据库密码与回报 HMAC 密钥通过 secrets 注入并随每次部署写入服务器 `/etc/toolforge-plugin-stats/runtime.env`（`root:toolforge-stats 0640`）；`DB_PASSWORD` 建议只用字母数字（DSN 中不能安全携带 `@` 与引号）。Secrets 未就绪时应失败，不能静默改为未签名包、空历史目录、HTTP 下载或直接 COS 降级。

## 插件发布与历史搬迁

Plugins workflow 固定 COSCLI **v1.0.8 Linux amd64**，固定官方 GitHub release 资产 SHA-256 `7165f2ae16c5f7ac495864c963ca574a76e04ec72680d7bc8a8eee3234d8cf91`。升级须重新核实官方发行文件/摘要和 CLI 参数。上传配置在临时 0600 YAML 中，凭据不作为命令行参数、不输出；结束删除配置。COSCLI 关闭日志和失败请求文件。参考 [官方发行资产](https://github.com/tencentyun/coscli/releases/expanded_assets/v1.0.8)、[官方 cp 实现与参数](https://github.com/tencentyun/coscli/blob/v1.0.8/cmd/cp.go)。

首次从 `dev` 手动运行 Plugins 并选择 `bootstrap=true`：

1. 校验仓库访问权限，完整下载固定 `plugins` Release 的全部历史 `.tfp`。网络/鉴权/读取失败停止；仅已确认的 Release 404 可以作为无历史版本。
2. 原字节搬迁历史包，逐包检查官方签名；不重新构建/重签，不导入历史 GitHub 下载量。
3. 新版本在无生产凭据、只读 token 的独立 job 构建；干净 runner 只构建受信 xtask 并下载候选产物再单独签名，不运行插件 Cargo/Vite。签名私钥只提供给签名步骤，构建进程无法替换另一台 runner 的签名工具。
4. 全版本目录保留旧撤销/下架状态；缺历史文件、同版本内容冲突或签名异常停止。
5. `packages/{id}/{version}/{sha256}.tfp` 使用 COS `--forbid-overwrite`；已有对象只接受大小与 SHA-256 一致。HTTPS 自定义域名读回必须匹配。
6. 上传按摘要命名的不可变 catalog/registry 快照，导入 MySQL 目录，再更新短缓存 catalog/registry 指针。
7. 最后补 GitHub 兼容镜像；`.tfp` 永不 `--clobber`，仅两个 JSON 允许替换。

后续由 `dev` 相关路径推送触发。插件、共享 SDK、前端锁文件或该插件 WASM 依赖闭包中的 Cargo.lock 记录发生变化但版本没升时失败；App 原生依赖独有的 Cargo.lock 变化不会要求市场插件升版，不能覆盖现有版本。首次 COS catalog 404 只在显式 bootstrap 下允许，403/网络失败永远停止。全版本目录使用 Plugins workflow 的 `GITHUB_RUN_NUMBER` 作递增 generation；不要重建/更名该 workflow 后重置计数。迁移其他仓库或调整 generation 时先确认现有服务代次，不能绕过过期发布保护。

失败重跑会重新读取历史；COS 目录中已有但 GitHub 尚未镜像的版本从 COS 取回原始签名字节，不重新构建。已成功上传的同摘要 COS 对象跳过，GitHub 已镜像版本不会覆盖。同一 generation 仅接受目录内容完全相同的幂等重跑。目录已导入而索引未发出不会删除制品，重新发布会对账。GitHub 镜像中断时 COS 正式索引可能已经生效；重跑补镜像，不回滚成功统计。旧 App 可以跟随 302，但无法提交成功回报，完整口径从新 App 起生效。

## 部署和回滚

推送 `dev` 且涉及服务端路径时自动执行：先 Go 格式、依赖、vet、race 单元/真实 MySQL 集成检查，再构建 `CGO_ENABLED=0` Linux amd64 二进制并带提交 SHA。通过固定 known_hosts 的受限 SSH 按字节流上传，服务器不安装 Go。并发由 `plugin-stats-production` 组串行化，不会交错部署。

需要与 CI 相同产物的本地构建用 `deploy/plugin-stats/build.sh`（默认完整测试，`--skip-tests` 跳过；输出二进制与 `plugin-stats.sha256`，`-X main.buildSHA` 注入与 workflow 一致——缺失会导致 `/readyz` 校验失败并触发回滚）。

服务端入口依次获得同一把部署/导入锁、验证 SHA-256、安装不可变版本目录、使用独立迁移角色执行 `migrate up`、原子切换 `current`、重启、轮询本机 `/readyz` 的 `buildSha`。失败恢复旧链接并检查旧服务；首次无旧版则停止失败服务。数据库迁移绝不自动 down。旧二进制兼容迁移是回滚前提。

人工回滚应使用已保留旧制品，通过受限部署入口提交旧 SHA 与对应摘要；不要手工执行破坏性数据库回滚。当前代码不提供删除历史制品/统计数据的自动任务。索引回滚从 `snapshots/registry/<sha256>.json` 恢复已知良好内容，仍需保持统计 API URL；撤销恶意版本应先更新可信目录状态，不能只把索引降版。

验收包括新服务首次启动、新版失败自动回退、并发回报计数幂等、签名/摘要错误零成功量、App 重启继续回报、API 故障不直连 COS、目录旧 generation 拒绝、同版本不同制品拒绝、撤销不复活及发布中断重跑。跨系统不存在事务，MySQL 目录、COS 指针、GitHub 镜像的各阶段要分开验证。
