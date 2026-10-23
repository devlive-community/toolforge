export const PLUGIN_INDEX_URL = 'https://github.com/devlive-community/toolforge/releases/download/plugins/registry.json'
const versionPattern = /^\d+\.\d+\.\d+$/
export function comparePluginVersions(a, b) {
  const x = a.split('.').map(Number),
    y = b.split('.').map(Number)
  for (let i = 0; i < 3; i++) {
    if (x[i] !== y[i]) return x[i] > y[i] ? 1 : -1
  }
  return 0
}
export function parsePluginIndex(value) {
  if (!value || !Array.isArray(value.plugins)) throw new Error('Invalid plugin registry')
  const versions = new Map()
  for (const entry of value.plugins) {
    if (!/^org\.devlive\.toolforge\.[a-z0-9][a-z0-9-]*$/.test(entry?.id ?? '') || !versionPattern.test(entry.version ?? '') || entry.signed !== true) continue
    const slug = entry.id.slice('org.devlive.toolforge.'.length)
    const existing = versions.get(slug)
    if (!existing || comparePluginVersions(entry.version, existing.version) > 0) versions.set(slug, { version: entry.version })
  }
  return versions
}
export async function loadPluginIndex({ fetchImpl = fetch, warn = console.warn } = {}) {
  try {
    const response = await fetchImpl(PLUGIN_INDEX_URL, { signal: AbortSignal.timeout(12000), headers: { 'User-Agent': 'toolforge-website' } })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
    return parsePluginIndex(await response.json())
  } catch (error) {
    warn(`[generate-data] plugin registry unavailable (${error.message})`)
    return null
  }
}
export function marketReleaseStatus(slug, sourceVersion, index, hasStableBuiltin = false) {
  if (index === null) return { status: 'released', releaseVerified: false }
  const published = index.get(slug)
  if (!published) return { status: 'upcoming', releaseVerified: true, ...(hasStableBuiltin ? { updateOnly: true, builtinFallback: true } : {}) }
  const pending = versionPattern.test(sourceVersion) && comparePluginVersions(sourceVersion, published.version) > 0
  return { status: pending ? 'upcoming' : 'released', ...(pending ? { updateOnly: true } : {}), releaseVerified: true, publishedVersion: published.version }
}
