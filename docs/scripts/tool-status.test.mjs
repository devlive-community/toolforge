import test from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { collectToolReleaseStatus, toolReleaseStatus } from './tool-status.mjs'

// Real Git trees, without commits, tags, network or any repository history.
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'toolforge-tool-status-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const git = (args, input) => execFileSync('git', args, { cwd: root, encoding: 'utf8', input, stdio: ['pipe', 'pipe', 'pipe'] })
  git(['init', '--quiet'])
  const tree = (entries) => {
    const records = Object.entries(entries).map(([name, value]) => {
      const directory = typeof value !== 'string'
      const hash = directory ? tree(value) : git(['hash-object', '-w', '--stdin'], value).trim()
      return `${directory ? '040000 tree' : '100644 blob'} ${hash}\t${name}\0`
    })
    return git(['mktree', '-z'], records.join('')).trim()
  }
  return { git, snapshot: (plugins, readme = 'unchanged') => tree({ plugins, 'README.md': readme }) }
}

const plugin = (code, extra = {}) => ({ 'manifest.json': '{"version":"0.1.0"}', backend: { 'lib.rs': code }, ...extra })

test('counts a shipped code update even when manifest versions are identical', (t) => {
  const { git, snapshot } = fixture(t)
  const previousRef = snapshot({ ocr: plugin('image only'), stable: plugin('stable') })
  const latestRef = snapshot({ ocr: plugin('image + PDF'), stable: plugin('stable'), added: plugin('new') })
  const sets = collectToolReleaseStatus({ previousRef, latestRef, currentRef: latestRef, git })
  assert.deepEqual(toolReleaseStatus('ocr', sets), { status: 'updated' })
  assert.deepEqual(toolReleaseStatus('added', sets), { status: 'new' })
  assert.deepEqual(toolReleaseStatus('stable', sets), { status: 'released' })
  const counts = ['ocr', 'stable', 'added'].reduce((counts, slug) => {
    counts[toolReleaseStatus(slug, sets).status]++
    return counts
  }, { released: 0, new: 0, updated: 0, upcoming: 0 })
  assert.deepEqual(counts, { released: 1, new: 1, updated: 1, upcoming: 0 })
})

test('unreleased code changes take priority, including changes to newly shipped tools', (t) => {
  const { git, snapshot } = fixture(t)
  const previousRef = snapshot({ ocr: plugin('old') })
  const latestRef = snapshot({ ocr: plugin('PDF'), added: plugin('first release') })
  const currentRef = snapshot({ ocr: plugin('PDF fix'), added: plugin('next change'), future: plugin('not shipped') })
  const sets = collectToolReleaseStatus({ previousRef, latestRef, currentRef, git })
  assert.deepEqual(toolReleaseStatus('ocr', sets), { status: 'upcoming', updateOnly: true })
  assert.deepEqual(toolReleaseStatus('added', sets), { status: 'upcoming', updateOnly: true })
  assert.deepEqual(toolReleaseStatus('future', sets), { status: 'upcoming' })
})

test('UI, locale, resource and file deletions count as plugin changes', (t) => {
  const { git, snapshot } = fixture(t)
  const original = {
    ui: plugin('same', { ui: { 'index.tsx': 'old UI' } }),
    locale: plugin('same', { locales: { 'en-US.json': 'old label' } }),
    resource: plugin('same', { 'icon.svg': 'old icon' }),
    deletion: plugin('same', { 'obsolete.txt': 'removed in release' }),
  }
  const changed = {
    ui: plugin('same', { ui: { 'index.tsx': 'new UI' } }),
    locale: plugin('same', { locales: { 'en-US.json': 'new label' } }),
    resource: plugin('same', { 'icon.svg': 'new icon' }),
    deletion: plugin('same'),
  }
  const latestRef = snapshot(changed)
  const sets = collectToolReleaseStatus({ previousRef: snapshot(original), latestRef, currentRef: latestRef, git })
  assert.deepEqual([...sets.updatedSet].sort(), ['deletion', 'locale', 'resource', 'ui'])
})

test('outside changes and reverted content do not mark unchanged plugins as updated', (t) => {
  const { git, snapshot } = fixture(t)
  const plugins = { stable: plugin('same') }
  const sets = collectToolReleaseStatus({ previousRef: snapshot(plugins, 'old docs'), latestRef: snapshot(plugins, 'new docs'), currentRef: snapshot(plugins, 'next docs'), git })
  assert.deepEqual(toolReleaseStatus('stable', sets), { status: 'released' })
  assert.equal(sets.updatedSet.size, 0)
  assert.equal(sets.pendingSet.size, 0)
})

test('manifest-only updates still count and removed tools are not pending updates', (t) => {
  const { git, snapshot } = fixture(t)
  const previousRef = snapshot({ metadata: plugin('same'), removed: plugin('old') })
  const latestRef = snapshot({ metadata: plugin('same', { 'manifest.json': '{"version":"0.1.1"}' }), removed: plugin('old') })
  const currentRef = snapshot({ metadata: plugin('same', { 'manifest.json': '{"version":"0.1.1"}' }) })
  const sets = collectToolReleaseStatus({ previousRef, latestRef, currentRef, git })
  assert.deepEqual(toolReleaseStatus('metadata', sets), { status: 'updated' })
  assert.equal(sets.pendingSet.has('removed'), false)
})

test('missing Git data keeps the conservative released fallback', () => {
  assert.throws(() => collectToolReleaseStatus({ previousRef: 'missing', latestRef: 'missing', git: () => { throw new Error('missing tree') } }), /missing tree/)
  assert.deepEqual(toolReleaseStatus('ocr', null), { status: 'released' })
})
