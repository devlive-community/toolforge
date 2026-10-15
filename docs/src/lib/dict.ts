// 站点界面文案（页面骨架文字；工具名与描述来自 manifest 双语数据）
export const dict = {
  nav: {
    home: { zh: '首页', en: 'Home' },
    tools: { zh: '全部工具', en: 'Tools' },
    changelog: { zh: '更新日志', en: 'Changelog' },
    github: { zh: 'GitHub', en: 'GitHub' },
  },
  hero: {
    badge: { zh: '全部本地运行 · 数据不出设备', en: 'Runs locally · Data never leaves your machine' },
    title: { zh: '桌面上常备的', en: 'The desktop toolbox' },
    titleAccent: { zh: '工具箱', en: 'that stays local' },
    subtitle: {
      zh: 'ToolForge 把常用的开发与效率工具做成插件：识别、转换、编码、网络、系统信息……65 个工具开箱即用，全部在本地 Rust 中运行。',
      en: 'ToolForge ships everyday developer and productivity tools as plugins: OCR, converters, encoding, network, system info — 65 tools out of the box, all running locally in Rust.',
    },
    download: { zh: '下载 ToolForge', en: 'Download ToolForge' },
    browse: { zh: '浏览全部工具', en: 'Browse all tools' },
    viewSource: { zh: '查看源码', en: 'View source' },
  },
  download: {
    macos: { zh: 'macOS', en: 'macOS' },
    windows: { zh: 'Windows', en: 'Windows' },
    linux: { zh: 'Linux', en: 'Linux' },
    appleSilicon: { zh: 'Apple Silicon', en: 'Apple Silicon' },
    intel: { zh: 'Intel', en: 'Intel' },
    x64: { zh: 'x64', en: 'x64' },
    arm64: { zh: 'ARM64', en: 'ARM64' },
    appimage: { zh: 'AppImage', en: 'AppImage' },
    deb: { zh: 'deb', en: 'deb' },
    version: { zh: '最新版本', en: 'Latest version' },
    other: { zh: '前往 Releases 下载', en: 'Download from Releases' },
    hint: {
      zh: '安装包由 minisign 签名，应用内置自动更新。',
      en: 'Installers are minisign-signed; the app updates itself.',
    },
  },
  stats: {
    tools: { zh: '个内置工具', en: 'built-in tools' },
    categories: { zh: '个分类', en: 'categories' },
    uploads: { zh: '次数据上传', en: 'data uploads' },
  },
  features: {
    title: { zh: '为什么是 ToolForge', en: 'Why ToolForge' },
    privacyTitle: { zh: '本地优先', en: 'Local first' },
    privacyBody: {
      zh: '识别、转换、解析全部在本地 Rust 完成，文件与数据不上传任何服务器。',
      en: 'OCR, conversion and parsing run in local Rust — files and data never touch a server.',
    },
    pluginTitle: { zh: '一切皆插件', en: 'Everything is a plugin' },
    pluginBody: {
      zh: '每个工具独立打包，随装随用；按分类与关键词检索，侧栏一键直达。',
      en: 'Every tool is an independent plugin — searchable by category and keyword, one click away in the sidebar.',
    },
    openTitle: { zh: '开源可查', en: 'Open source' },
    openBody: {
      zh: '全部源码在 GitHub，构建流水线公开，安装包带签名校验。',
      en: 'All sources on GitHub with a public pipeline and signed installers.',
    },
  },
  toolsPage: {
    title: { zh: '全部工具', en: 'All tools' },
    subtitle: {
      zh: '按分类筛选或直接搜索；点击工具查看详情。',
      en: 'Filter by category or search; open a tool for details.',
    },
    search: { zh: '搜索工具、功能或关键词…', en: 'Search tools, features or keywords…' },
    all: { zh: '全部', en: 'All' },
    count: { zh: '个工具', en: 'tools' },
    empty: { zh: '没有匹配的工具，换个关键词试试。', en: 'No tools match — try another keyword.' },
  },
  detail: {
    back: { zh: '返回全部工具', en: 'Back to all tools' },
    keywords: { zh: '关键词', en: 'Keywords' },
    category: { zh: '分类', en: 'Category' },
    howTo: { zh: '如何在应用中使用', en: 'How to use it in the app' },
    steps: {
      one: { zh: '安装并打开 ToolForge。', en: 'Install and open ToolForge.' },
      two: { zh: '在左侧栏搜索或浏览到这个工具。', en: 'Find the tool in the sidebar (search by name).' },
      three: { zh: '打开即可使用，数据全部留在本机。', en: 'Open it — your data stays on the machine.' },
    },
    notFound: { zh: '没有找到这个工具。', en: 'This tool could not be found.' },
  },
  changelog: {
    title: { zh: '更新日志', en: 'Changelog' },
    subtitle: {
      zh: '来自 GitHub Releases 的正式发布记录。',
      en: 'Published releases from GitHub.',
    },
    viewAll: { zh: '在 GitHub 查看全部', en: 'View all on GitHub' },
    empty: {
      zh: '暂时没有发布记录，请稍后再看。',
      en: 'No published releases yet — check back later.',
    },
  },
  footer: {
    built: { zh: '用 Next.js 构建 · 部署于 GitHub Pages', en: 'Built with Next.js · Deployed on GitHub Pages' },
    license: { zh: '开源许可', en: 'License' },
  },
  theme: {
    toggle: { zh: '切换深浅色', en: 'Toggle theme' },
  },
} as const

export type Dict = typeof dict
