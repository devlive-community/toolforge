<div align="center">
<img src="apps/desktop/src-tauri/icons/logo.svg" width="96" />

<h1>ToolForge</h1>

**快速 · 安全 · 轻量 · 插件化的开发者与效率工具桌面平台**

基于 Tauri 2 与 Rust 构建：所有数据都在本地由 Rust 处理，每个工具都是独立插件，
现代化的界面，支持亮 / 暗主题、中英文与在线自动更新。

<p>
  <a href="https://github.com/devlive-community/toolforge/releases"><img alt="release" src="https://img.shields.io/github/v/release/devlive-community/toolforge?include_prereleases&label=release" /></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-555" />
  <img alt="stack" src="https://img.shields.io/badge/Tauri%202-React%2019-FFC131" />
  <a href="https://github.com/devlive-community/toolforge/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/devlive-community/toolforge/actions/workflows/ci.yml/badge.svg" /></a>
  <img alt="license" src="https://img.shields.io/badge/license-MIT-green" />
</p>

<p><a href="README.md">English</a> · <strong>简体中文</strong></p>
</div>

---

## ✨ 特性

- **数据只在本地处理**：解析、格式化、编解码、哈希、文件读写全部由 Rust 完成，前端只负责展示，数据不会离开本机。
- **一切皆插件**：每个工具（包括内置工具）都是独立插件，拥有自己的 manifest、Rust 后端、React 界面与语言包，宿主不写死任何工具。
- **耗时任务实时反馈**：大文件等耗时操作以后台任务运行，实时显示进度与日志，可随时取消；全局任务中心可查看历史与日志。
- **现代化界面**：语义化设计 Token、亮 / 暗主题、自绘组件与窗口控件、⌘K 命令面板、收藏与最近使用。
- **识别剪贴板的 ⌘K**：打开命令面板时自动识别复制的内容（JSON、JWT、时间戳、UUID、颜色、curl 命令、cron 表达式、IP、SQL、电子表格、证书、截图等），推荐对应的工具并直接带入内容；识别在本地由 Rust 完成，可随时关闭。
- **快捷启动**：全局快捷键（默认 ⌥Space / Alt+Space，可自定义）在任意应用中唤起 ToolForge 与命令面板；开启后台运行后常驻菜单栏 / 系统托盘。
- **国际化**：简体中文与英文，插件自带语言包；Rust 只返回错误码，由界面翻译。
- **资源按需下载**：AI 模型等大文件只在工具需要时下载，来源经校验、支持断点续传，可随时删除。
- **在线自动更新**：基于 GitHub Releases，更新包经签名校验，可跳过版本或关闭自动检查。
- **本地存储只用 SQLite**：偏好、收藏、使用记录与任务历史都保存在本机数据库。

## 🧰 内置工具

| 工具 | 说明 |
|------|------|
| **JSON 格式化** | 格式化、压缩、校验、转义 / 反转义、树形视图、结构化对比；支持 JSON5；错误定位到行列；大文件树形视图按需加载 |
| **XML 格式化** | 格式化、压缩、校验；保留注释、CDATA 与声明；未闭合、错配标签精确到行列 |
| **SQL 格式化** | 美化与压缩；关键字大小写、缩进、紧凑模式；通用 SQL / PostgreSQL / SQL Server 方言 |
| **正则测试** | 实时高亮匹配与分组、替换预览；支持断言、反向引用与命名分组；常用正则预设 |
| **时间戳转换** | 秒 / 毫秒 / 微秒 / 纳秒自动识别；多时区对照；日期字符串转时间戳；实时时钟 |
| **UUID 生成** | UUID v1 / v3 / v4 / v5 / v7、ULID、NanoID 批量生成；解析版本与内含时间 |
| **哈希计算** | MD5、SHA-1、SHA-2、SHA3、SM3、CRC32；文本实时计算与 HMAC 校验；文件拖入批量计算，任务化运行并显示实时日志 |
| **编解码** | Base64 / Base64URL / Base32 / Hex、URL（组件、完整 URL、表单）、HTML 实体、Unicode 转义双向转换；文件转 Base64 / Data URI |
| **JWT 解析** | 解析 Header 与 Payload、判断过期、校验 HS / RS / PS / ES / EdDSA 签名并签发新令牌 |
| **证书查看** | 解析 PEM / DER 证书与证书链（名称、备用名称、有效期、公钥、用途、指纹），或获取服务器实际下发的证书链并用系统信任库校验，显示 TLS 版本与加密套件 |
| **加解密** | AES（GCM / CBC / CTR / ECB）、国密 SM4 与 ChaCha20-Poly1305，支持十六进制、Base64、文本或 PBKDF2 口令密钥；RSA 密钥生成、OAEP / PKCS#1 加解密与 PKCS#1 / PSS 签名验签，与 OpenSSL 兼容 |
| **文本处理** | 13 种命名风格与大小写转换；去空白、去重、排序（自然排序、按长度）、打乱与行号；字符、词数、中日韩字数与阅读时长统计 |
| **文本对比** | 按行 / 按词 / 按字符对比，左右并排高亮，可忽略大小写与空白，导出 unified 补丁 |
| **格式转换** | JSON ⇄ YAML ⇄ TOML ⇄ CSV，自动识别输入格式、保持键顺序，支持 CSV 分隔符、表头与类型推断 |
| **进制转换** | 任意精度整数在 2–36 进制间转换，自动识别前缀，8–128 位补码与可点击的 64 位视图 |
| **颜色转换** | HEX / RGB / HSL / HSV / HWB / CMYK / Lab / OKLCH 互转，WCAG 对比度检查，明暗色阶与配色 |
| **密码生成** | 密码学安全的随机密码，可按类型选择与排除字符，并在本地检查强度与破解时间 |
| **二维码** | 生成 PNG / SVG 二维码，可设置纠错等级、尺寸与颜色；识别图片中的二维码 |
| **去除背景** | 使用本地 AI 模型（U²-Net 轻量版或 IS-Net）在 Rust 中抠图；模型首次使用时下载，支持续传与 SHA-256 校验；输出透明 PNG 或纯色背景 |
| **文字识别** | 使用 PaddleOCR PP-OCRv4 模型在 Rust 中离线识别图片与截图中的中英文文字（首次使用下载约 15 MB 模型）；支持 ⌘V 粘贴截图，显示文字框并可点击复制单行 |
| **IP 计算器** | IPv4 / IPv6 子网：网络与广播地址、主机范围、地址数量、地址类型与反向解析；子网拆分、归属检查与地址段转 CIDR |
| **HTTP 请求** | 发送带参数、请求头与 JSON / 文本 / 表单请求体的请求；查看状态、耗时、响应头与格式化的 JSON；保存响应、复制为 cURL |
| **DNS 查询** | 通过系统解析器或公共 DNS 查询 A / AAAA / CNAME / MX / TXT / NS / SOA / SRV / CAA / PTR 记录，并排对比结果与耗时；识别代理的 fake-ip |
| **端口管理** | 查看每个 TCP / UDP 端口被哪个进程占用（含用户、内存与命令行），支持搜索与结束进程；并行检查远程端口是否可达并显示延迟与失败原因 |
| **本地服务** | 把文件夹作为网站在本机或局域网内访问（附二维码），实时查看每个请求；支持目录列表、SPA 回退、CORS 与断点续传，默认隐藏点文件，路径无法跳出所选文件夹 |
| **系统监控** | 实时查看 CPU、内存、磁盘、网络与温度，以及可搜索的进程列表 |
| **单位换算** | 长度、重量、温度、面积、体积、数据大小与速率、压强、能量、功率等 14 类单位，含市制单位 |
| **图片压缩** | 离线批量压缩 JPEG（mozjpeg）、PNG（带透明度的抖动调色板或 oxipng 无损）与 WebP；可缩放、移除 EXIF、保留色彩配置，不覆盖原图，并可拖动分隔线与原图对比 |
| **图片转换** | PNG / JPEG / WebP / GIF / BMP / ICO / TIFF 批量互转，按比例或限定宽高缩放，JPEG 质量可调；以任务运行并输出实时日志，不覆盖已有文件 |
| **Cron 表达式** | 逐字段解释 5 / 6 / 7 段 Cron 表达式，按任意时区预览后续执行时间，支持 `L`、`W`、`#` 与 `@daily` 等别名 |
| **JSON 转代码** | 从 JSON 样例生成 TypeScript、Rust（serde）、Go、Java record、Kotlin、Python（pydantic）与 C# 模型，自动合并数组并识别可选与可空字段 |
| **cURL 转代码** | 把 curl 命令（含浏览器「复制为 cURL」的 bash / cmd 格式）转换为 JavaScript fetch、Python requests、Go、Rust reqwest、Java HttpClient、PHP 与 C# 代码，无法转换的选项会逐条列出 |
| **模拟数据** | 生成最多 10 万行逼真的中文或英文测试数据（姓名与邮箱一致、校验码正确的身份证号、真实城市等 35 种类型），输出为 JSON、CSV 或 SQL INSERT，可用种子复现 |
| **CSV 查看器** | 打开最大 1 GB 的 CSV / TSV 文件（百万行不到一秒），自动识别编码、分隔符与表头；支持排序、搜索、筛选、列统计，并可导出为 CSV / TSV / JSON / Markdown |
| **日志查看** | 秒开 GB 级日志，按级别、文本或正则筛选并高亮，像 `tail -f` 一样实时跟踪新行，查看行内 JSON；支持 GB18030 编码并去除颜色控制码 |
| **图片信息** | 格式、尺寸、颜色类型、EXIF 拍摄参数与 GPS 位置（可在地图中打开）、主色提取；无损移除 JPEG / PNG 中的 EXIF / XMP / IPTC |
| **Markdown 编辑器** | GitHub 风格 Markdown，Rust 实时渲染预览，目录、字数统计、格式工具栏与快捷键；草稿自动保存，可导出独立 HTML |

更多工具持续开发中。

## 📦 下载安装

前往 [Releases](https://github.com/devlive-community/toolforge/releases) 下载对应平台的安装包：

| 平台 | 安装包 |
|------|--------|
| macOS（Apple Silicon / Intel） | `.dmg` |
| Windows | `.msi` / `-setup.exe` |
| Linux | `.AppImage` / `.deb` / `.rpm` |

> 当前版本尚未进行系统代码签名。macOS 首次打开如提示「已损坏」或「无法验证开发者」，可右键选择「打开」，或执行：
>
> ```bash
> xattr -cr /Applications/ToolForge.app
> ```
>
> Windows 如出现 SmartScreen 提示，选择「更多信息 → 仍要运行」。应用内自动更新不受影响。

## 🛠 开发

### 环境要求

- [Node.js](https://nodejs.org/) 22+ 与 [pnpm](https://pnpm.io/) 10+
- [Rust](https://rustup.rs/) stable（edition 2024）
- Tauri 2 的[系统依赖](https://tauri.app/start/prerequisites/)（Linux 需要 `libwebkit2gtk-4.1-dev` 等）

### 常用命令

```bash
pnpm install          # 安装前端依赖
pnpm dev              # 启动桌面应用（开发模式，热更新）
pnpm build            # 构建安装包

cargo xtask check     # 运行与 CI 完全一致的全部检查
cargo xtask check rust|web|rules   # 只运行其中一部分
pnpm lint             # ESLint（零警告）
```

`cargo xtask check` 依次执行：约定扫描 → rustfmt → clippy → Rust 测试 → TypeScript 类型检查 → ESLint → 前端构建。

### 插件结构

```
plugins/<name>/
├── manifest.json          # id（org.devlive.toolforge.<name>）、分类、函数声明、权限
├── icon.svg
├── locales/zh-CN.json     # 插件文案、日志与错误码翻译
├── backend/               # Rust：实现 ToolPlugin，所有数据处理在这里
└── ui/                    # React：只负责展示，通过 usePlugin / useTask 调用后端
```

- 普通函数通过 `call()` 调用，适合即时处理；manifest 中声明 `"task": true` 的函数必须以任务运行，通过 `TaskContext` 上报日志、进度并响应取消。
- 界面只能使用 `@toolforge/ui` 组件与语义 Token；文件、剪贴板等能力通过 SDK 的 `host` 使用。

### 开发约定

- 数据处理必须在 Rust 侧完成；前端禁止引入数据处理库、禁止使用浏览器存储（由 ESLint 与约定扫描保证）。
- 样式只使用语义 Token，不写原始色板与 `dark:`；不使用原生表单控件与对话框。
- 界面文案全部走 i18n；Rust 只返回错误码与参数。
- Rust 测试放在独立的 `xxx_test.rs` 文件中。
- 提交遵循 [Conventional Commits](https://www.conventionalcommits.org/)，每个功能独立提交。

## 🚀 发布

```bash
scripts/release.sh --dry-run        # 预览发布计划与发布说明
scripts/release.sh                  # 发布当前版本
scripts/release.sh minor --wait     # 升级 minor 后发布，并等待 CI 完成
scripts/release.sh 1.0.0 --next none
scripts/release.sh --help           # 查看全部选项
```

发布脚本会预检仓库状态、运行全部检查、创建附注标签（内容为该版本的提交历史）并推送；
GitHub Actions 随后构建 macOS / Windows / Linux 安装包，全部成功后正式发布 Release 与自动更新清单，
脚本最后把版本号更新为下一个开发版本并提交。

发布前需在**仓库** Secrets 中配置更新器签名私钥 `TOOLFORGE_UPDATER_PRIVATE_KEY`（私钥文件内容，须与 `tauri.conf.json` 中的公钥配对），私钥设有密码时再配置 `TOOLFORGE_UPDATER_KEY_PASSWORD`。

## 📄 License

[MIT](https://opensource.org/licenses/MIT) © devlive-community
