// 构建前数据管线：扫描 plugins/*/manifest.json 与双语 locales，生成工具目录；
// 拉取 GitHub Releases 注入下载链接与更新日志。API 限流时读取公开发布页；均不可用时回退到 Releases 链接。
//
// 产物（全部 gitignore，构建时重新生成）：
//   src/data/catalog.json   工具目录（双语）
//   src/data/release.json   最新版本与各平台安装包链接（可为 null）
//   src/data/releases.json  最近若干条发布记录
//   public/icons/<slug>.svg 插件图标
//   public/icon.png         站点 favicon

import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { loadReleaseData } from './release-data.mjs'

const root = resolve(import.meta.dirname, '..', '..')
const pluginsDir = join(root, 'plugins')
const outDataDir = resolve(import.meta.dirname, '..', 'src', 'data')
const outIconsDir = resolve(import.meta.dirname, '..', 'public', 'icons')

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


  // ── 发布状态：对比最新两个版本 tag 中 plugins/ 的目录集合，不依赖 commit 信息 ──
// 最新 tag 有、上个 tag 没有 → new（随最新版发布）
// 当前工作区有、最新 tag 没有 → upcoming（已合入但未随任何版本发布）
// 两个 tag 都有 → released
// 只依赖 git：CI 浅克隆时按需浅 fetch 两个 tag；任何一步失败返回 null，
// 此时所有工具视为已发布（数据不可用时宁可不标，也不整站误标）

// 宽松 semver 比较：剥离 v 前缀后按数字段逐位比较
function compareVersions(a, b) {
  const pa = a.replace(/^v/i, '').split('.').map((part) => Number.parseInt(part, 10) || 0)
  const pb = b.replace(/^v/i, '').split('.').map((part) => Number.parseInt(part, 10) || 0)
  for (let index = 0; index < Math.max(pa.length, pb.length); index += 1) {
    const delta = (pa[index] ?? 0) - (pb[index] ?? 0)
    if (delta !== 0) return delta
  }
  return 0
}

function gitOutput(args) {
  // git 命令必须在仓库根执行：ls-tree 的 plugins/ pathspec 相对 cwd 解析
  return execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()
}

function releaseStatusSets() {
  try {
    const tags = [...gitOutput(['ls-remote', '--tags', '--refs', 'origin']).matchAll(/refs\/tags\/(v\d[\w.-]*)$/gm)]
      .map((match) => match[1])
      .sort((a, b) => compareVersions(a, b))
    if (tags.length < 2) return null
    const [prevTag, latestTag] = tags.slice(-2) // 升序排序后末尾两个为 [次新, 最新]
    // CI checkout 通常是浅克隆且没有 tag 引用：按显式 refspec 浅拉两个 tag
    // （git fetch <tag> 只写 FETCH_HEAD 不建本地引用，后续 ls-tree 会找不到）
    execFileSync(
      'git',
      [
        'fetch', '--depth=1', '--quiet', '--no-tags', 'origin',
        `+refs/tags/${latestTag}:refs/tags/${latestTag}`,
        `+refs/tags/${prevTag}:refs/tags/${prevTag}`,
      ],
      { cwd: root, stdio: 'ignore' },
    )
    const pluginsAt = (tag) =>
      new Set(
        gitOutput(['ls-tree', '--name-only', tag, 'plugins/'])
          .split('\n')
          .filter(Boolean)
          .map((line) => line.replace(/^plugins\//, '').replace(/\/$/, '')),
      )
    const latestSet = pluginsAt(latestTag)
    const prevSet = pluginsAt(prevTag)

    // ── 插件更新检测：约定插件更新时递增 manifest.json 的 version ──
    // 先用 git diff 缩小到有文件变化的插件，再精确比较 manifest version
    const slugsTouchedBetween = (from, to) => {
      const out = new Set()
      for (const line of gitOutput(['diff', '--name-only', from, to, '--', 'plugins/']).split('\n')) {
        const slug = line.split('/')[1]
        if (slug) out.add(slug)
      }
      return out
    }
    const manifestVersionAt = (tag, slug) => {
      try {
        return JSON.parse(gitOutput(['show', `${tag}:plugins/${slug}/manifest.json`])).version ?? ''
      } catch {
        return null // 读不到时不参与判定，避免误标
      }
    }

    // 两个 tag 都有、version 变化 → updated（随最新版更新发布）
    const updatedSet = new Set()
    for (const slug of slugsTouchedBetween(prevTag, latestTag)) {
      if (!latestSet.has(slug) || !prevSet.has(slug)) continue // 新增/删除不算更新
      const latest = manifestVersionAt(latestTag, slug)
      if (latest !== null && latest !== manifestVersionAt(prevTag, slug)) updatedSet.add(slug)
    }

    // HEAD 的 manifest version 相对最新 tag 变化 → 有未发布更新；记录最新 tag 时的 version
    const latestVersions = new Map()
    for (const slug of slugsTouchedBetween(latestTag, 'HEAD')) {
      if (!latestSet.has(slug)) continue // 新插件由目录集合判定
      const version = manifestVersionAt(latestTag, slug)
      if (version !== null) latestVersions.set(slug, version)
    }

    return { latestTag, latestSet, prevTag, prevSet, updatedSet, latestVersions }
  } catch (error) {
    console.warn(`[generate-data] release status unavailable (${String(error.message).split('\n')[0]}), treating all tools as released`)
    return null
  }
}

function scanPlugins(sets) {
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
    // 当前 manifest version 比最新 tag 新 → 有已合并未发布的更新
    const updateOnly =
      sets !== null &&
      sets.latestSet.has(slug) &&
      sets.latestVersions.get(slug) !== undefined &&
      sets.latestVersions.get(slug) !== manifest.version
    // - upcoming：当前目录最新 tag 没有（新工具未发布），或有未发布的更新
    // - new：最新 tag 有、上个 tag 没有（随最新版首次发布）
    // - updated：两个 tag 都有但 version 变化（随最新版更新发布）
    // - released：其余
    const status = !sets
      ? 'released'
      : updateOnly || !sets.latestSet.has(slug)
        ? 'upcoming'
        : !sets.prevSet.has(slug)
          ? 'new'
          : sets.updatedSet.has(slug)
            ? 'updated'
            : 'released'
    tools.push({
      slug,
      name,
      description,
      category,
      categoryLabel: CATEGORY_LABELS[category] ?? { zh: category, en: category },
      keywords: manifest.keywords ?? [],
      status,
      ...(updateOnly ? { updateOnly: true } : {}),
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

mkdirSync(outDataDir, { recursive: true })
mkdirSync(outIconsDir, { recursive: true })

const release = await loadReleaseData()
if (!release.fallback) console.log(`[generate-data] latest release: ${release.version}`)
const sets = releaseStatusSets()
if (sets) console.log(`[generate-data] status tags: ${sets.prevTag} -> ${sets.latestTag}`)
const tools = scanPlugins(sets)
const counts = { released: 0, new: 0, updated: 0, upcoming: 0 }
for (const tool of tools) counts[tool.status] += 1
console.log(`[generate-data] status: ${counts.released} released, ${counts.new} new, ${counts.updated} updated, ${counts.upcoming} upcoming`)

writeFileSync(join(outDataDir, 'catalog.json'), JSON.stringify(tools, null, 2))
writeFileSync(join(outDataDir, 'release.json'), JSON.stringify(release, null, 2))

// favicon：直接复用桌面端应用图标
const icon128 = join(root, 'apps/desktop/src-tauri/icons/128x128@2x.png')
if (existsSync(icon128)) {
  writeFileSync(resolve(import.meta.dirname, '..', 'public', 'icon.png'), readFileSync(icon128))
}

console.log(`[generate-data] ${tools.length} tools indexed`)
