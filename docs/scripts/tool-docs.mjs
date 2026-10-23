export function localizedText(value, locale) {
  if (typeof value !== 'string') return ''
  if (!value.startsWith('i18n:')) return value
  let current = locale
  for (const key of value.slice(5).split('.')) current = current && typeof current === 'object' ? current[key] : undefined
  return typeof current === 'string' ? current : ''
}

export function buildToolDocs(manifest, locales) {
  const permissions = Array.isArray(manifest.permissions) ? manifest.permissions.filter((v) => typeof v === 'string') : []
  const functions = []
  for (const [name, spec] of Object.entries(manifest.functions ?? {})) {
    const declaration = spec?.interface
    if (
      !declaration ||
      typeof declaration !== 'object' ||
      Array.isArray(declaration) ||
      !Array.isArray(declaration.expose) ||
      !['read', 'write', 'external'].includes(declaration.effect) ||
      typeof declaration.summary !== 'string' ||
      !declaration.params ||
      declaration.params.type !== 'object' ||
      (declaration.confirm !== undefined && typeof declaration.confirm !== 'boolean')
    )
      continue
    const uses = declaration.uses ?? []
    if (!Array.isArray(uses) || uses.some((p) => typeof p !== 'string' || !permissions.includes(p))) continue
    const expose = declaration.expose.filter((channel) => channel === 'cli' || channel === 'agent')
    if (!expose.length) continue
    const summaries = Object.fromEntries(Object.entries(locales).map(([lang, locale]) => [lang, localizedText(declaration.summary, locale)]))
    const params = Object.entries(declaration.params.properties ?? {})
      .filter(([, schema]) => schema && typeof schema === 'object' && !Array.isArray(schema))
      .map(([key, schema]) => ({
        name: key,
        type: typeof schema.type === 'string' ? schema.type : '',
        required: Array.isArray(declaration.params.required) && declaration.params.required.includes(key),
        descriptions: Object.fromEntries(Object.entries(locales).map(([lang, locale]) => [lang, localizedText(schema.description, locale)])),
        sensitive: schema['x-sensitive'] === true,
      }))
    functions.push({
      name,
      expose,
      summaries,
      effect: declaration.effect,
      uses,
      confirm: declaration.confirm === true,
      task: spec.task === true,
      params,
      examples: Array.isArray(declaration.examples) ? declaration.examples.filter((v) => typeof v === 'string') : [],
    })
  }
  return {
    permissions,
    minAppVersion: typeof manifest.minAppVersion === 'string' ? manifest.minAppVersion : '',
    platforms: Array.isArray(manifest.platforms) ? manifest.platforms.filter((p) => ['macos', 'windows', 'linux'].includes(p)) : [],
    sensitive: manifest.sensitive === true,
    help: Object.fromEntries(
      Object.entries(locales).map(([lang, locale]) => [lang, Array.isArray(locale.help) ? locale.help.filter((v) => typeof v === 'string' && v.trim()) : []]),
    ),
    functions,
  }
}
