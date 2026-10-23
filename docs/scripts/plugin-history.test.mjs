import test from 'node:test'
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { collectPluginHistory, groupHistory, parseSubject } from './plugin-history.mjs'

test('parses conventional commit subjects', () => {
  assert.deepEqual(parseSubject('feat(hash): add HMAC'), { type: 'feat', scope: 'hash', text: 'add HMAC' })
  assert.deepEqual(parseSubject('fix!: drop old flag'), { type: 'fix', scope: '', text: 'drop old flag' })
  assert.equal(parseSubject('Merge branch dev'), null)
})

test('groups commits by manifest version and keeps user-facing changes', () => {
  const history = groupHistory(
    [
      { hash: 'c'.repeat(40), date: '2026-10-23', subject: 'fix(x): repair output', version: '0.1.2' },
      { hash: 'b'.repeat(40), date: '2026-10-22', subject: 'feat(x): add export', version: '0.1.2' },
      { hash: 'a'.repeat(40), date: '2026-10-01', subject: 'style(x): format', version: '0.1.1' },
    ],
    '0.1.1',
  )
  assert.equal(history.length, 2)
  assert.deepEqual(history[0], {
    version: '0.1.2',
    date: '2026-10-22',
    pending: true,
    changes: [
      { type: 'fix', text: 'repair output', hash: 'ccccccc' },
      { type: 'feat', text: 'add export', hash: 'bbbbbbb' },
    ],
  })
  assert.deepEqual(history[1], { version: '0.1.1', date: '2026-10-01', pending: false, changes: [] })
  assert.equal(groupHistory([{ hash: 'a'.repeat(40), date: 'd', subject: 'feat: x', version: '2.0.0' }])[0].pending, false)
})

test('follows a plugin that moved from plugins/ to market/', (t) => {
  const root = mkdtempSync(join(tmpdir(), 'toolforge-plugin-history-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const git = (args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] })
  const commit = (message) => {
    git(['add', '-A'])
    git(['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '--quiet', '-m', message])
  }
  git(['init', '--quiet'])
  mkdirSync(join(root, 'plugins/demo'), { recursive: true })
  writeFileSync(join(root, 'plugins/demo/manifest.json'), '{"version":"0.1.0"}')
  commit('feat(demo): add demo')
  mkdirSync(join(root, 'market'), { recursive: true })
  renameSync(join(root, 'plugins/demo'), join(root, 'market/demo'))
  writeFileSync(join(root, 'market/demo/manifest.json'), '{"version":"0.1.1"}')
  commit('feat(demo): move to market')

  const history = collectPluginHistory('demo', { git, released: '0.1.0' })
  assert.deepEqual(
    history.map((h) => [h.version, h.pending, h.changes.map((c) => c.text)]),
    [
      ['0.1.1', true, ['move to market']],
      ['0.1.0', false, ['add demo']],
    ],
  )
  assert.deepEqual(collectPluginHistory('missing', { git }), [])
})
