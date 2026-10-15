// The API is preferred. Public GitHub release pages remain usable when its API is rate-limited.
const REPOSITORY = 'devlive-community/toolforge'
const ORIGIN = 'https://github.com'
const RELEASES = `${ORIGIN}/${REPOSITORY}/releases`

export function matchAssets(assets) {
  const pick = (pattern) => assets.find((asset) => pattern.test(asset.name))?.browser_download_url
  return {
    macosApple: pick(/(?:aarch64|arm64).*\.dmg$/i),
    macosIntel: pick(/(?:x64|intel).*\.dmg$/i),
    windowsX64: pick(/x64.*setup\.exe$/i),
    windowsArm: pick(/arm64.*setup\.exe$/i),
    linuxAppImage: pick(/amd64\.appimage$/i),
    linuxDeb: pick(/amd64\.deb$/i),
  }
}

function decodeEntities(value) {
  const named = { amp: '&', quot: '"', apos: "'", lt: '<', gt: '>', nbsp: ' ' }
  return value.replace(/&(#x[0-9a-f]+|#\d+|amp|quot|apos|lt|gt|nbsp);/gi, (all, entity) => {
    if (!entity.startsWith('#')) return named[entity.toLowerCase()] ?? all
    const numeric = entity.toLowerCase().startsWith('#x')
      ? Number.parseInt(entity.slice(2), 16) : Number.parseInt(entity.slice(1), 10)
    return numeric >= 0 && numeric <= 0x10ffff ? String.fromCodePoint(numeric) : all
  })
}

// Convert the limited Markdown-rendered markup in GitHub's release feed, without rendering raw HTML.
export function releaseHtmlToMarkdown(html) {
  return decodeEntities(html
    .replace(/<(script|style|svg)\b[^>]*>[\s\S]*?<\/\1>/gi, '')
    .replace(/<!--[^]*?-->/g, '')
    .replace(/<pre\b[^>]*>([\s\S]*?)<\/pre>/gi, (_, text) => `\n\n\`\`\`\n${text.replace(/<[^>]*>/g, '')}\n\`\`\`\n\n`)
    .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, (_, level, text) => `\n\n${'#'.repeat(Number(level))} ${text}\n\n`)
    .replace(/<(?:code|tt)\b[^>]*>([\s\S]*?)<\/(?:code|tt)>/gi, '`$1`')
    .replace(/<a\b[^>]*href="([^"]*)"[^>]*>([\s\S]*?)<\/a>/gi, (_, href, text) => {
      try {
        const url = new URL(decodeEntities(href), ORIGIN)
        return ['https:', 'http:'].includes(url.protocol) ? `[${text}](${url.href})` : text
      } catch { return text }
    })
    .replace(/<(?:strong|b)\b[^>]*>([\s\S]*?)<\/(?:strong|b)>/gi, '**$1**')
    .replace(/<(?:em|i)\b[^>]*>([\s\S]*?)<\/(?:em|i)>/gi, '*$1*')
    .replace(/<li\b[^>]*>/gi, '\n- ')
    .replace(/<\/(?:ul|ol|p)>/gi, '\n\n')
    .replace(/<br\s*\/?\s*>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim())
}

function stableVersion(tag) {
  return /^v?\d+\.\d+\.\d+$/.test(tag) ? tag.replace(/^v/, '').split('.').map(Number) : null
}
function atMost(tag, latest) {
  const a = stableVersion(tag), b = stableVersion(latest)
  if (!a || !b) return false
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return a[index] < b[index]
  }
  return true
}

export function parseReleaseFeed(xml, latestTag) {
  const entries = [...xml.matchAll(/<entry>([\s\S]*?)<\/entry>/g)]
  return entries.flatMap(([, entry]) => {
    const href = entry.match(/<link\b[^>]*href="([^"]+)"/)?.[1]
    if (!href?.startsWith(`${RELEASES}/tag/`)) return []
    const tag = decodeURIComponent(href.slice(`${RELEASES}/tag/`.length))
    if (!atMost(tag, latestTag)) return []
    const title = entry.match(/<title>([\s\S]*?)<\/title>/)?.[1]
    const content = entry.match(/<content\b[^>]*>([\s\S]*?)<\/content>/)?.[1] ?? ''
    return [{ tag, name: decodeEntities(title ?? tag), date: (entry.match(/<updated>([^<]+)/)?.[1] ?? '').slice(0, 10), body: releaseHtmlToMarkdown(decodeEntities(content)) }]
  }).slice(0, 10)
}

export function parseAssetLinks(html, tag) {
  const prefix = `/${REPOSITORY}/releases/download/${encodeURIComponent(tag)}/`
  const paths = [...html.matchAll(/href="([^"]+)"/g)].map(([, href]) => decodeEntities(href))
  const urls = [...new Set(paths.flatMap((href) => {
    try {
      const url = new URL(href, ORIGIN)
      return url.origin === ORIGIN && url.pathname.startsWith(prefix) ? [url.href] : []
    } catch { return [] }
  }))]
  return urls.map((url) => ({ name: decodeURIComponent(new URL(url).pathname.split('/').pop()), browser_download_url: url }))
}

function releaseBody(html) {
  const open = /<div\b[^>]*data-test-selector="body-content"[^>]*>/.exec(html)
  if (!open) return ''
  const start = open.index + open[0].length
  const tags = /<\/?div\b[^>]*>/g
  tags.lastIndex = start
  let depth = 1
  for (let match = tags.exec(html); match; match = tags.exec(html)) {
    depth += match[0].startsWith('</') ? -1 : 1
    if (!depth) return releaseHtmlToMarkdown(html.slice(start, match.index))
  }
  return ''
}

export async function loadReleaseData({ fetchImpl = fetch, token = process.env.GITHUB_TOKEN, warn = console.warn } = {}) {
  const fetchedAt = new Date().toISOString()
  const get = async (url, api = false) => {
    const headers = { 'User-Agent': 'toolforge-website' }
    // Credentials only go to the API, never to HTML pages or asset downloads.
    if (api && token) headers.Authorization = `Bearer ${token}`
    const response = await fetchImpl(url, { headers, signal: AbortSignal.timeout(12000) })
    if (!response.ok) throw new Error(`HTTP ${response.status}`)
    return response
  }
  try {
    const response = await get(`https://api.github.com/repos/${REPOSITORY}/releases?per_page=15`, true)
    const list = await response.json()
    const published = list.filter((item) => !item.draft && !item.prerelease)
    const latest = published[0]
    if (!latest?.tag_name) throw new Error('No stable release returned')
    return {
      version: latest.tag_name.replace(/^v/, ''), url: latest.html_url,
      assets: matchAssets(latest.assets ?? []), fetchedAt,
      releases: published.slice(0, 10).map((item) => ({ tag: item.tag_name, name: item.name || item.tag_name, date: (item.published_at ?? '').slice(0, 10), body: item.body ?? '' })),
    }
  } catch (error) {
    warn(`[generate-data] release API unavailable (${error.message}); checking public release page`)
  }
  try {
    const response = await get(`${RELEASES}/latest`)
    const url = new URL(response.url)
    const pathPrefix = `/${REPOSITORY}/releases/tag/`
    if (url.origin !== ORIGIN || !url.pathname.startsWith(pathPrefix)) throw new Error('Unrecognized latest release URL')
    const tag = decodeURIComponent(url.pathname.slice(pathPrefix.length))
    if (!stableVersion(tag)) throw new Error('Unrecognized stable release version')
    const html = await response.text()
    const assetsResponse = await get(`${RELEASES}/expanded_assets/${encodeURIComponent(tag)}`)
    const assets = matchAssets(parseAssetLinks(await assetsResponse.text(), tag))
    let releases = []
    try { releases = parseReleaseFeed(await (await get(`${RELEASES}.atom`)).text(), tag) }
    catch (error) { warn(`[generate-data] release feed unavailable (${error.message})`) }
    const feedLatest = releases.find((item) => item.tag === tag)
    const latest = { tag, name: feedLatest?.name || tag, date: (html.match(/<relative-time\b[^>]*datetime="([^"]+)"/)?.[1] ?? '').slice(0, 10) || feedLatest?.date || '', body: releaseBody(html) || feedLatest?.body || '' }
    // The latest redirect is authoritative, including when the Atom feed is briefly delayed.
    releases = [latest, ...releases.filter((item) => item.tag !== tag)].slice(0, 10)
    return { version: tag.replace(/^v/, ''), url: url.href, assets, releases, fetchedAt }
  } catch (error) {
    warn(`[generate-data] public release page unavailable (${error.message}); using Releases link`)
    return { version: '', url: `${RELEASES}/latest`, assets: {}, releases: [], fetchedAt, fallback: true }
  }
}
