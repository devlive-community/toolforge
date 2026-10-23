// Keep the publisher's wording; remove commit noise rather than inventing translations.
export function summarizeRelease(body) {
  const result = { features: [], fixes: [], other: [], hidden: 0 }
  let kind = 'other'
  for (const line of String(body ?? '').split('\n')) {
    if (/^#{1,6}\s/.test(line)) {
      kind = /feature|新增|新功能/i.test(line) ? 'features' : /fix|修复/i.test(line) ? 'fixes' : 'other'
      continue
    }
    if (!/^\s*[-*]\s+/.test(line)) continue
    const text = line
      .replace(/^\s*[-*]\s+/, '')
      .replace(/\*\*|`/g, '')
      .replace(/\[([0-9a-f]{7,40})\]\([^)]*\)/gi, '')
      .replace(/\(([0-9a-f]{7,40})\)/gi, '')
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/^[\w.-]+:\s*/, '')
      .replace(/\(\s*\)/g, '')
      .trim()
    if (!text) continue
    if (result[kind].length < 3) result[kind].push(text)
    else result.hidden++
  }
  return result
}
