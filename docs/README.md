# ToolForge 官网

ToolForge 桌面工具箱的官方网站：双语工具目录、平台下载入口与更新日志。

- 线上地址：<https://toolforge.devlive.org>
- 部署：GitHub Actions → GitHub Pages（静态导出）
- 数据源：`plugins/*/manifest.json` + 各插件 `locales/`，构建时扫描生成——**新增插件无需改网站**

## 快速开始

### 环境要求

- Node.js 22+
- pnpm 10（根目录 `packageManager` 已声明；建议 `corepack enable` 或直接用 `corepack pnpm`）

### 安装与运行

```bash
cd docs
corepack pnpm install
corepack pnpm dev      # 开发服务器
corepack pnpm build    # 静态导出到 out/
corepack pnpm test     # 发布数据与直达下载回退的回归检查
```

> `docs` 是独立于根 workspace 的自包含站点（自带 `pnpm-workspace.yaml` 与锁文件），
> 依赖变动不会影响桌面端，桌面端依赖变动也不会破坏站点构建。

## 目录说明

```
docs/
├── scripts/generate-data.mjs   # 构建前数据管线（见下）
├── src/
│   ├── app/[lang]/             # zh | en 双语路由（首页 / 工具目录 / 详情 / 更新日志）
│   ├── components/             # Header / Footer / ToolCard / ToolsBrowser / DownloadButtons 等
│   ├── lib/catalog.ts          # 类型与数据加载（catalog.json / release.json）
│   ├── lib/dict.ts             # 界面文案（中英）
│   └── styles/globals.css      # Tailwind v4 + 设计 tokens（暗色优先）
├── public/
│   ├── CNAME                   # toolforge.devlive.org
│   ├── index.html              # 根路径按浏览器语言跳转
│   └── icons/<slug>.svg        # 生成：各插件图标
└── src/data/*.json             # 生成：工具目录 / 发布数据（gitignore）
```

## 数据管线

`scripts/generate-data.mjs` 在 `build` / `dev` 前自动执行：

1. 扫描 `plugins/*/manifest.json` 与 `locales/{zh-CN,en-US}.json`，对比最新两个版本 tag 自动判定各工具的发布状态，生成 `src/data/catalog.json`
2. 复制各插件 `icon.svg` 到 `public/icons/<slug>.svg`
3. 拉取 GitHub Releases：最新版本号、各平台安装包链接（dmg / setup.exe / AppImage / deb）与最近 10 条发布记录
   - 有 `GITHUB_TOKEN` 时不受匿名限流影响；API 限流时自动读取 GitHub 最新正式发布页、公开安装包清单与发布订阅源；二者都不可用才回退到 Releases 页面链接，且不把开发版本标为最新正式版

## 发布状态（待发布 / 新发布 / 已更新）

完全自动判定，**不需要维护任何清单、不修改插件代码**。`generate-data.mjs` 通过 git 对比发布状态：

- `git ls-remote --tags` 取最新两个版本 tag（如 `v0.1.9`、`v0.1.8`），`git ls-tree` 读取两个 tag 和 HEAD 中各插件的目录树指纹，不依赖插件版本号递增或提交信息
- 当前目录有、最新 tag 没有的插件 → **待发布**（已合入但未随任何版本发布）
- 最新 tag 中已有的插件，HEAD 目录内容继续变化 → **待发布更新**（稳定版基础功能仍可用）
- 最新 tag 有、上个 tag 没有的插件 → **新**（随最新版本首次发布）
- 两个 tag 都有、插件目录内容变化 → **已更新**（包含后端、界面、本地化、资源与文件删除；即使 manifest version 未变也能识别）
- 其余 → 已发布，无角标。插件目录之外的站点/发布说明改动不会把工具误标为更新

工具卡片、详情页共用同一状态角标，目录页提供「全部状态 / 已发布 / 新发布 / 已更新 / 待发布」筛选，最新一条发布记录在更新日志页标「最新」。待发布更新优先于新发布/已更新；git 数据不可用时全部视为已发布，构建不失败。

## 部署

- 推送到 `main` / `dev`、发布 Release、每日定时或手动触发都会运行 `.github/workflows/site-deploy.yml`
- 仓库设置中需启用 Pages（Source: GitHub Actions）并绑定自定义域名 `toolforge.devlive.org`
- DNS：为 `toolforge.devlive.org` 添加 CNAME 记录指向 `<org>.github.io`，并在 Pages 设置中开启 Enforce HTTPS

## 技术栈

Next.js 15（App Router · 静态导出） · React 19 · Tailwind CSS v4 · TypeScript · GitHub Actions

## 全站页面与下载

- 首页首屏直接展示最新正式版本的 macOS / Windows / Linux 安装包，按架构与格式一键下载；按钮地址来自真实发布资产，不拼接安装包文件名。
- 工具目录支持关键词、分类、发布状态叠加筛选与每页 15 项分页，工具卡片链接到双语详情页。
- 工具详情对尚未发布的新工具使用发布日志入口；已有工具的未发布更新与稳定版基础功能分开说明。
- 发布日志显示真实版本时间线、原文发布说明与可展开历史版本。
- 各页均支持深浅主题、手机布局与正确的双语 canonical；手机顶部可直接进入工具目录和发布日志。
- `scripts/release-data.mjs` 独立负责发布 API / 公开页面回退；单元测试覆盖 API 限流、缺失安装包、版本过滤与完全离线。
