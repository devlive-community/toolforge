import assert from 'node:assert/strict'
import test from 'node:test'
import { createExportSession, saveCurrentOutput } from '../src/exportSession.ts'

test('editing while the save dialog is open never writes an obsolete preview', async () => {
  const session = createExportSession()
  let resolvePath
  const path = new Promise((resolve) => { resolvePath = resolve })
  const writes = []
  const saving = saveCurrentOutput('old rules', session.capture(), () => path, async (...args) => { writes.push(args) })
  session.invalidate()
  resolvePath('/project/.gitignore')
  assert.equal(await saving, 'changed')
  assert.deepEqual(writes, [])
})

test('leaving the tool while the dialog is open revokes the export', async () => {
  const session = createExportSession()
  let resolvePath
  const path = new Promise((resolve) => { resolvePath = resolve })
  const saving = saveCurrentOutput('rules', session.capture(), () => path, async () => { assert.fail('must not write') })
  session.close()
  session.activate()
  resolvePath('/project/.gitignore')
  assert.equal(await saving, 'changed')
})

test('cancelling the dialog does not write or report an error', async () => {
  const session = createExportSession()
  assert.equal(await saveCurrentOutput('rules', session.capture(), async () => null, async () => { assert.fail('must not write') }), 'cancelled')
})

test('an unchanged export preserves the backend output and line endings exactly', async () => {
  const session = createExportSession()
  const output = '# Template\r\nnode_modules/\r\n\\!keep\r\n'
  const writes = []
  assert.equal(await saveCurrentOutput(output, session.capture(), async () => '/project/.gitignore', async (...args) => { writes.push(args) }), 'saved')
  assert.deepEqual(writes, [['/project/.gitignore', output]])
})

test('write failures remain visible to the caller', async () => {
  const session = createExportSession()
  const failure = new Error('write failed')
  await assert.rejects(saveCurrentOutput('rules', session.capture(), async () => '/project/.gitignore', async () => { throw failure }), failure)
})
