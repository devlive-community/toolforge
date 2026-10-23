// 插件版本历史：按 git 提交把 manifest.json 中 version 相同的提交归为同一个版本，
// 只保留面向用户的 feat / fix / perf 提交说明（保留原文）。插件从 plugins/ 迁到 market/ 后历史连续。
// 浅克隆或 git 不可用时返回空历史，不影响站点构建。

const USER_FACING = new Set(['feat', 'fix', 'perf'])
const SUBJECT = /^(\w+)(?:\(([^)]*)\))?(!)?:\s*(.+)$/
export const MAX_VERSIONS = 20

/** 解析提交标题；不是 Conventional Commits 格式时返回 null */
export function parseSubject(subject) {
  const match = SUBJECT.exec(subject.trim())
  if (!match) return null
  return { type: match[1], scope: match[2] ?? '', text: match[4] }
}

const numeric = (version) => version.split('.').map((part) => Number.parseInt(part, 10) || 0)
function compare(a, b) {
  const x = numeric(a),
    y = numeric(b)
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0)
  }
  return 0
}

/**
 * commits：从新到旧，每项 { hash, date, subject, version }（version 为该提交时 manifest 的版本）。
 * released：已发布的最高版本，更高的版本标记为待发布；为 null 时全部视为已发布。
 */
export function groupHistory(commits, released = null) {
  const versions = new Map()
  for (const commit of commits) {
    if (!commit.version) continue
    let entry = versions.get(commit.version)
    if (!entry) {
      entry = { version: commit.version, date: commit.date, changes: [] }
      versions.set(commit.version, entry)
    }
    // 从新到旧遍历：最后看到的是该版本最早的提交，作为版本日期
    entry.date = commit.date
    const parsed = parseSubject(commit.subject)
    if (parsed && USER_FACING.has(parsed.type)) {
      entry.changes.push({ type: parsed.type, text: parsed.text, hash: commit.hash.slice(0, 7) })
    }
  }
  return [...versions.values()]
    .sort((a, b) => compare(b.version, a.version))
    .slice(0, MAX_VERSIONS)
    .map((entry) => ({ ...entry, pending: released !== null && compare(entry.version, released) > 0 }))
}

/** 读取某个插件的提交与各提交时的版本；git 为执行 git 命令并返回输出的函数 */
export function collectPluginHistory(slug, { git, released = null }) {
  const dirs = [`market/${slug}`, `plugins/${slug}`]
  let log
  try {
    log = git(['log', '--format=%H%x1f%ad%x1f%s', '--date=short', '--', ...dirs])
  } catch {
    return []
  }
  const commits = []
  for (const line of log.split('\n')) {
    if (!line.trim()) continue
    const [hash, date, subject] = line.split('\x1f')
    let version = null
    for (const dir of dirs) {
      try {
        version = JSON.parse(git(['show', `${hash}:${dir}/manifest.json`])).version ?? null
        break
      } catch {
        /* 该提交时插件不在这个目录 */
      }
    }
    commits.push({ hash, date, subject, version })
  }
  return groupHistory(commits, released)
}
