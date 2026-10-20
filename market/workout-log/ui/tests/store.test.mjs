import assert from 'node:assert/strict'
import test from 'node:test'
import { apply, errorCause, PersistenceError, recover, request, serial, transact } from '../src/store.ts'
function fixture() {
  const data = new Map()
  return { data, read: async (key) => data.get(key) ?? null, write: async (key, value) => { if (value === null) data.delete(key); else data.set(key, value) }, call: async (_fn, args) => args }
}
test('a failed multi-key write is replayed before the next reader', async () => {
  const s = fixture()
  const write = s.write
  let failed = false
  s.write = async (key, value) => { if (key === 'workout-1' && !failed) { failed = true; throw new Error('disk') } await write(key, value) }
  await assert.rejects(apply(s, { need: [], data: null, writes: { head: { nextWorkout: 2 }, 'workout-1': { saved: true } } }))
  assert.ok(s.data.has('pending'))
  await recover(s)
  assert.deepEqual(s.data.get('workout-1'), { saved: true })
  assert.equal(s.data.has('pending'), false)
})
test('failed validation never creates a durable pending write', async () => {
  const s = fixture()
  s.call = async () => { throw new Error('invalid') }
  await assert.rejects(apply(s, { need: [], writes: { head: {} }, data: null }))
  assert.equal(s.data.size, 0)
})
test('transaction queue spans remounts and a rejected call does not poison it', async () => {
  const s = fixture()
  let release
  const wait = new Promise((resolve) => { release = resolve })
  const a = serial('same-plugin', async () => { await wait; await s.write('head', 'first') })
  const b = serial('same-plugin', async () => { assert.equal(await s.read('head'), 'first'); throw new Error('rejected') })
  release()
  await a
  await assert.rejects(b)
  await serial('same-plugin', async () => assert.equal(await s.read('head'), 'first'))
})
test('missing or corrupt storage is never replaced by a successful empty fallback', async () => {
  const s = fixture()
  s.read = async () => { throw new Error('unreadable') }
  await assert.rejects(transact('read-failure', s, {}))
  assert.equal(s.data.size, 0)
})
test('validation remains editable and a corrected operation can run without reloading', async () => {
  const s = fixture()
  const invalid = { code: 'workout.incompleteSet', params: {} }
  s.call = async (_fn, { action }) => {
    if (action.invalid) throw invalid
    return { need: [], writes: {}, data: { saved: true } }
  }
  await assert.rejects(transact('validation', s, { invalid: true }), (error) => {
    assert.equal(error, invalid)
    assert.equal(error instanceof PersistenceError, false)
    assert.equal(errorCause(error), invalid)
    return true
  })
  assert.equal(s.data.size, 0)
  assert.deepEqual(await transact('validation', s, { invalid: false }), { saved: true })
})
test('unreadable pending, head and requested records retain a persistence failure cause', async () => {
  for (const broken of ['pending', 'head', 'workout-1']) {
    const s = fixture()
    const original = { code: 'store.failed', params: { detail: broken } }
    const read = s.read
    s.read = async (key) => { if (key === broken) throw original; return read(key) }
    s.call = async (_fn, { records }) => records['workout-1']
      ? { need: [], writes: {}, data: true }
      : { need: ['workout-1'], writes: {}, data: null }
    await assert.rejects(transact(`read-${broken}`, s, {}), (error) => {
      assert.ok(error instanceof PersistenceError)
      assert.equal(errorCause(error), original)
      return true
    })
    assert.equal(s.data.size, 0)
  }
})
test('recovery validation failure is blocking and leaves the pending journal intact', async () => {
  const s = fixture()
  const pending = { writes: { head: { version: 99 } } }
  s.data.set('pending', pending)
  const cause = { code: 'workout.invalid', params: { field: 'stored' } }
  s.call = async () => { throw cause }
  await assert.rejects(recover(s), (error) => {
    assert.ok(error instanceof PersistenceError)
    assert.equal(errorCause(error), cause)
    return true
  })
  assert.equal(s.data.get('pending'), pending)
})
test('a write failure is distinguishable from validation and is replayed before new edits', async () => {
  const s = fixture()
  const write = s.write
  const cause = { code: 'store.failed', params: { detail: 'fixture disk' } }
  let interrupted = false
  s.write = async (key, value) => {
    if (key === 'workout-1' && !interrupted) { interrupted = true; throw cause }
    await write(key, value)
  }
  await assert.rejects(apply(s, { need: [], writes: { head: { version: 1 }, 'workout-1': { saved: true } }, data: null }), (error) => {
    assert.ok(error instanceof PersistenceError)
    assert.equal(errorCause(error), cause)
    return true
  })
  s.call = async (fn, args) => {
    if (fn === 'recover') return args
    assert.deepEqual(s.data.get('workout-1'), { saved: true })
    assert.equal(s.data.has('pending'), false)
    return { need: [], writes: {}, data: 'editable' }
  }
  assert.equal(await transact('after-disk-failure', s, {}), 'editable')
})
test('direct export/import requests also classify underlying read failures', async () => {
  const s = fixture()
  const cause = new Error('fixture head unavailable')
  s.read = async () => { throw cause }
  await assert.rejects(request(s, {}, 'import', { text: '{}' }), (error) => {
    assert.ok(error instanceof PersistenceError)
    assert.equal(errorCause(error), cause)
    return true
  })
})
