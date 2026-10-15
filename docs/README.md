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

1. 扫描 `plugins/*/manifest.json` 与 `locales/{zh-CN,en-US}.json`，生成 `src/data/catalog.json`
2. 复制各插件 `icon.svg` 到 `public/icons/<slug>.svg`
3. 拉取 GitHub Releases：最新版本号、各平台安装包链接（dmg / setup.exe / AppImage / deb）与最近 10 条发布记录
   - 有 `GITHUB_TOKEN` 时不受匿名限流影响；拉取失败自动回退为 Releases 页面链接，构建不失败

## 部署

- 推送到 `main` / `dev`、发布 Release、每日定时或手动触发都会运行 `.github/workflows/site-deploy.yml`
- 仓库设置中需启用 Pages（Source: GitHub Actions）并绑定自定义域名 `toolforge.devlive.org`
- DNS：为 `toolforge.devlive.org` 添加 CNAME 记录指向 `<org>.github.io`，并在 Pages 设置中开启 Enforce HTTPS

## 技术栈

Next.js 15（App Router · 静态导出） · React 19 · Tailwind CSS v4 · TypeScript · GitHub Actions
