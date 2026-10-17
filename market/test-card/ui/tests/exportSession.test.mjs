import assert from 'node:assert/strict'
import test from 'node:test'
import { createExportSession, saveCurrentOutput } from '../src/exportSession.ts'

function deferred() {
  let resolve
  const promise = new Promise((done) => { resolve = done })
  return { promise, resolve }
}

for (const format of ['csv', 'json']) {
  test(`switching banks while the ${format.toUpperCase()} dialog is open never writes the previous bank's cards`, async () => {
    const session = createExportSession()
    const dialog = deferred()
    const writes = []
    const saving = saveCurrentOutput(`previous-bank.${format}`, session.capture(), () => dialog.promise, async (...args) => { writes.push(args) })

    session.invalidate()
    dialog.resolve(`/exports/test-cards.${format}`)

    assert.equal(await saving, 'changed')
    assert.deepEqual(writes, [])
  })
}

test('a new generation revokes earlier exports and allows only its own output', async () => {
  const session = createExportSession()
  const writes = []
  const revisions = [session.capture()]
  for (const input of ['brand', 'count', 'seed', 'regenerate']) {
    session.invalidate()
    assert.equal(revisions.at(-1)(), false, `${input} changes must revoke the earlier result`)
    revisions.push(session.capture())
  }

  for (const stale of revisions.slice(0, -1)) {
    assert.equal(await saveCurrentOutput('obsolete cards', stale, async () => { assert.fail('a stale export must not open a dialog') }, async () => { assert.fail('must not write') }), 'changed')
  }
  const output = 'purpose,bank,number\r\ntest-only,测试银行,0000000000000000\r\n'
  assert.equal(await saveCurrentOutput(output, revisions.at(-1), async () => '/exports/test-cards.csv', async (...args) => { writes.push(args) }), 'saved')
  assert.deepEqual(writes, [['/exports/test-cards.csv', output]])
})

test('cancelling a save leaves the output untouched without reporting a failure', async () => {
  const session = createExportSession()
  assert.equal(await saveCurrentOutput('cards', session.capture(), async () => null, async () => { assert.fail('must not write') }), 'cancelled')
})

test('write failures propagate to the caller', async () => {
  const session = createExportSession()
  const failure = new Error('disk is full')
  await assert.rejects(saveCurrentOutput('cards', session.capture(), async () => '/exports/test-cards.json', async () => { throw failure }), failure)
})

test('leaving and reactivating the tool revokes an open save from the old mount', async () => {
  const session = createExportSession()
  const dialog = deferred()
  const saving = saveCurrentOutput('old mount cards', session.capture(), () => dialog.promise, async () => { assert.fail('must not write') })

  session.close()
  session.activate()
  dialog.resolve('/exports/test-cards.csv')

  assert.equal(await saving, 'changed')
  const writes = []
  assert.equal(await saveCurrentOutput('current mount cards', session.capture(), async () => '/exports/current.csv', async (...args) => { writes.push(args) }), 'saved')
  assert.deepEqual(writes, [['/exports/current.csv', 'current mount cards']])
})

test('Strict Mode cleanup and setup cannot reactivate an earlier export revision', () => {
  const session = createExportSession()
  session.activate()
  const firstSetup = session.capture()
  session.close()
  assert.equal(firstSetup(), false)
  session.activate()
  assert.equal(firstSetup(), false)
  assert.equal(session.capture()(), true)
})
