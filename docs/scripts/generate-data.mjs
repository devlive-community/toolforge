// 构建前数据管线：扫描 plugins/*/manifest.json 与双语 locales，生成工具目录；
// 拉取 GitHub Releases 注入下载链接与更新日志。API 不可用时静默回退，构建永不失败。
//
// 产物（全部 gitignore，构建时重新生成）：
//   src/data/catalog.json   工具目录（双语）
//   src/data/release.json   最新版本与各平台安装包链接（可为 null）
//   src/data/releases.json  最近若干条发布记录
//   public/icons/<slug>.svg 插件图标
//   public/icon.png         站点 favicon

import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..', '..')
const pluginsDir = join(root, 'plugins')
const outDataDir = resolve(import.meta.dirname, '..', 'src', 'data')
const outIconsDir = resolve(import.meta.dirname, '..', 'public', 'icons')
const repo = 'devlive-community/toolforge'
const releasesPage = `https://github.com/${repo}/releases/latest`

// 分类标签：目录页的筛选与详情页展示用
const CATEGORY_LABELS = {
  dev: { zh: '开发', en: 'Development' },
  image: { zh: '图像', en: 'Images' },
  encode: { zh: '编解码', en: 'Encoding' },
  convert: { zh: '转换', en: 'Converters' },
  network: { zh: '网络', en: 'Network' },
  text: { zh: '文本', en: 'Text' },
  system: { zh: '系统', en: 'System' },
  calc: { zh: '计算', en: 'Calculators' },
  other: { zh: '其他', en: 'Other' },
}

function readLocale(pluginDir, file) {
  const path = join(pluginDir, 'locales', file)
  if (!existsSync(path)) return {}
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    return {}
  }
}

// manifest 里 name/description 是 i18n:name 占位符，真实文案在 locales 里
function localized(manifest, locale, field) {
  const value = locale?.[field]
  if (typeof value === 'string' && value && !value.startsWith('i18n:')) return value
  const fallback = manifest?.[field]
  return typeof fallback === 'string' && !fallback.startsWith('i18n:') ? fallback : ''
}

function scanPlugins() {
  const tools = []
  for (const entry of readdirSync(pluginsDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue
    const dir = join(pluginsDir, entry.name)
    const manifestPath = join(dir, 'manifest.json')
    if (!existsSync(manifestPath)) continue
    let manifest
    try {
      manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
    } catch (error) {
      console.warn(`[generate-data] skip ${entry.name}: ${error.message}`)
      continue
    }
    const slug = manifest.id.replace(/^org\.devlive\.toolforge\./, '') || entry.name
    const zh = readLocale(dir, 'zh-CN.json')
    const en = readLocale(dir, 'en-US.json')
    const name = { zh: localized(manifest, zh, 'name'), en: localized(manifest, en, 'name') }
    const description = {
      zh: localized(manifest, zh, 'description'),
      en: localized(manifest, en, 'description'),
    }
    if (!name.zh && !name.en) continue
    const category = manifest.category || 'other'
    tools.push({
      slug,
      name,
      description,
      category,
      categoryLabel: CATEGORY_LABELS[category] ?? { zh: category, en: category },
      keywords: manifest.keywords ?? [],
    })
    // 图标：复制到 public 供站点直接引用
    const icon = join(dir, 'icon.svg')
    if (existsSync(icon)) {
      writeFileSync(join(outIconsDir, `${slug}.svg`), readFileSync(icon))
    }
  }
  tools.sort((a, b) => (a.categoryLabel.en + a.name.en).localeCompare(b.categoryLabel.en + b.name.en))
  return tools
}

async function fetchJson(url, timeoutMs = 8000) {
  const headers = { 'User-Agent': 'toolforge-website' }
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`
  const response = await fetch(url, { headers, signal: AbortSignal.timeout(timeoutMs) })
  if (!response.ok) throw new Error(`${response.status}`)
  return response.json()
}

// 从资产名匹配平台安装包；tauri-action 的产物名形如
// ToolForge_0.1.10_aarch64.dmg / _x64-setup.exe / _amd64.AppImage
function matchAssets(assets) {
  const pick = (test) =>
    assets.find((asset) => test.test(asset.name))?.browser_download_url
  return {
    macosApple: pick(/aarch64.*\.dmg$|arm64.*\.dmg$/i),
    macosIntel: pick(/x64.*\.dmg$|intel.*\.dmg$/i),
    windowsX64: pick(/x64.*setup\.exe$|x64-setup\.exe$/i),
    windowsArm: pick(/arm64.*setup\.exe$/i),
    linuxAppImage: pick(/amd64\.appimage$/i),
    linuxDeb: pick(/amd64\.deb$/i),
  }
}

async function fetchReleases(appVersion) {
  try {
    const list = await fetchJson(`https://api.github.com/repos/${repo}/releases?per_page=15`)
    const published = list.filter((item) => !item.draft && !item.prerelease)
    const latest = published[0] ?? null
    const compact = published.slice(0, 10).map((item) => ({
      tag: item.tag_name,
      name: item.name || item.tag_name,
      date: (item.published_at ?? '').slice(0, 10),
      body: item.body ?? '',
    }))
    const data = {
      version: latest?.tag_name?.replace(/^v/, '') ?? appVersion,
      url: latest?.html_url ?? releasesPage,
      assets: latest ? matchAssets(latest.assets ?? []) : {},
      releases: compact,
      fetchedAt: new Date().toISOString(),
    }
    console.log(`[generate-data] latest release: ${data.version}`)
    return data
  } catch (error) {
    // 本地无令牌或网络受限时回退到 Releases 页面链接，构建不失败
    console.warn(`[generate-data] release fetch failed (${error.message}), using fallback`)
    return {
      version: appVersion,
      url: releasesPage,
      assets: {},
      releases: [],
      fetchedAt: new Date().toISOString(),
      fallback: true,
    }
  }
}

mkdirSync(outDataDir, { recursive: true })
mkdirSync(outIconsDir, { recursive: true })

const tools = scanPlugins()
const appVersion = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version
const release = await fetchReleases(appVersion)

writeFileSync(join(outDataDir, 'catalog.json'), JSON.stringify(tools, null, 2))
writeFileSync(join(outDataDir, 'release.json'), JSON.stringify(release, null, 2))

// favicon：直接复用桌面端应用图标
const icon128 = join(root, 'apps/desktop/src-tauri/icons/128x128@2x.png')
if (existsSync(icon128)) {
  writeFileSync(resolve(import.meta.dirname, '..', 'public', 'icon.png'), readFileSync(icon128))
}

console.log(`[generate-data] ${tools.length} tools indexed`)
