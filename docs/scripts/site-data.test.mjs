import test from 'node:test'
import assert from 'node:assert/strict'
import { buildToolDocs, localizedText } from './tool-docs.mjs'
import { loadPluginIndex, marketReleaseStatus, parsePluginIndex } from './plugin-releases.mjs'
import { summarizeRelease } from './release-summary.mjs'

const entry = (id, version, signed = true) => ({ id: `org.devlive.toolforge.${id}`, version, signed })
test('online release availability comes from signed published versions, using numeric ordering', () => {
  const index = parsePluginIndex({
    plugins: [entry('tool', '0.1.9'), entry('tool', '0.1.10'), entry('unsigned', '1.0.0', false), { id: 'foreign', version: '1.0.0', signed: true }],
  })
  assert.deepEqual(marketReleaseStatus('tool', '0.1.10', index), { status: 'released', releaseVerified: true, publishedVersion: '0.1.10' })
  assert.deepEqual(marketReleaseStatus('tool', '0.2.0', index), { status: 'upcoming', updateOnly: true, releaseVerified: true, publishedVersion: '0.1.10' })
  assert.deepEqual(marketReleaseStatus('unsigned', '1.0.0', index), { status: 'upcoming', releaseVerified: true })
})
test('a missing or invalid registry never claims an installable online version', async () => {
  const index = await loadPluginIndex({
    fetchImpl: async () => {
      throw new Error('offline')
    },
    warn: () => {},
  })
  assert.equal(index, null)
  assert.deepEqual(marketReleaseStatus('tool', '0.1.0', index), { status: 'released', releaseVerified: false })
  assert.throws(() => parsePluginIndex({ plugins: 'invalid' }), /Invalid/)
})

test('a tool migrating online keeps its stable built-in availability separate from online installation', () => {
  assert.deepEqual(marketReleaseStatus('json-formatter', '0.1.3', new Map(), true), {
    status: 'upcoming',
    releaseVerified: true,
    updateOnly: true,
    builtinFallback: true,
  })
})
test('registry loading reads the public registry without credentials', async () => {
  const index = await loadPluginIndex({
    fetchImpl: async (url, options) => {
      assert.match(url, /\/plugins\/registry\.json$/)
      assert.equal(options.headers.Authorization, undefined)
      return { ok: true, json: async () => ({ plugins: [entry('tool', '0.1.0')] }) }
    },
  })
  assert.equal(index.get('tool').version, '0.1.0')
})
test('tool docs preserve localized help, permissions, platforms and real interface declarations', () => {
  const manifest = {
    permissions: ['fs:user-selected'],
    sensitive: true,
    platforms: ['windows'],
    minAppVersion: '0.2.0',
    functions: {
      run: {
        task: true,
        interface: {
          expose: ['cli', 'agent'],
          summary: 'i18n:api.run',
          effect: 'write',
          uses: ['fs:user-selected'],
          confirm: true,
          params: { type: 'object', required: ['path'], properties: { path: { type: 'string', description: 'i18n:api.path', 'x-sensitive': true } } },
          examples: ['tfp-tool run --path example'],
        },
      },
    },
  }
  const docs = buildToolDocs(manifest, {
    zh: { help: ['真实用法', 5, null], api: { run: '运行', path: '路径' } },
    en: { help: ['Real usage'], api: { run: 'Run', path: 'Path' } },
  })
  assert.deepEqual(docs.help.zh, ['真实用法'])
  assert.deepEqual(docs.platforms, ['windows'])
  assert.equal(docs.sensitive, true)
  assert.equal(docs.functions[0].summaries.zh, '运行')
  assert.equal(docs.functions[0].params[0].sensitive, true)
  assert.equal(docs.functions[0].params[0].required, true)
})
test('undeclared, hidden, unknown or overprivileged interfaces are not advertised', () => {
  const common = { expose: ['agent'], summary: 'Read', params: { type: 'object' } }
  const docs = buildToolDocs(
    {
      permissions: [],
      functions: {
        absent: {},
        hidden: { interface: false },
        unknown: { interface: { ...common, effect: 'future' } },
        excessive: { interface: { ...common, effect: 'read', uses: ['network'] } },
        future: { interface: { ...common, effect: 'read', expose: ['future'] } },
      },
    },
    { zh: {}, en: {} },
  )
  assert.deepEqual(docs.functions, [])
  assert.equal(localizedText('i18n:missing.key', {}), '')
})
test('invalid parameter descriptions do not break the catalog build', () => {
  const docs = buildToolDocs(
    {
      functions: {
        read: {
          interface: { expose: ['cli'], effect: 'read', summary: 'Read', params: { type: 'object', properties: { valid: { type: 'string' }, broken: null } } },
        },
      },
    },
    { zh: {}, en: {} },
  )
  assert.deepEqual(
    docs.functions[0].params.map((p) => p.name),
    ['valid'],
  )
})
test('release summaries retain original wording and grouping without losing full-note context', () => {
  const body =
    '### Features\n- **tools:** add examples ([`123abcd`](https://example.test/commit/123abcd))\n### Fixes\n- parser: keep old records (abcdef0)\n### Improvements\n- Faster browsing\n- One\n- Two\n- Three'
  const summary = summarizeRelease(body)
  assert.deepEqual(summary.features, ['add examples'])
  assert.deepEqual(summary.fixes, ['keep old records'])
  assert.deepEqual(summary.other, ['Faster browsing', 'One', 'Two'])
  assert.equal(summary.hidden, 1)
  assert.equal(summarizeRelease('').features.length, 0)
})
