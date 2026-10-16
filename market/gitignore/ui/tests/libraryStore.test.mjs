import assert from 'node:assert/strict'
import test from 'node:test'
import { loadLibrary, mutateLibrary } from '../src/libraryStore.ts'

const deferred = () => {
  let resolve
  let reject
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

test('a remount waits for the pending durable write before reading', async () => {
  let saved = { templates: ['old'] }
  const writing = deferred()
  const enteredWrite = deferred()
  const mutation = mutateLibrary('remount', async () => saved, async () => ({ library: { templates: ['new'] } }), async (library) => {
    enteredWrite.resolve()
    await writing.promise
    saved = library
  })
  await enteredWrite.promise
  let read = false
  const mounted = loadLibrary('remount', async () => { read = true; return saved }, async (value) => value)
  await Promise.resolve()
  assert.equal(read, false)
  writing.resolve()
  await mutation
  assert.deepEqual(await mounted, { templates: ['new'] })
})

test('failed persistence never publishes the candidate and retries use the durable library', async () => {
  let saved = { templates: ['kept'] }
  const failure = new Error('database write failed')
  await assert.rejects(mutateLibrary('failure', async () => saved, async () => ({ library: { templates: ['not-saved'] } }), async () => { throw failure }), failure)
  assert.deepEqual(await loadLibrary('failure', async () => saved, async (value) => value), { templates: ['kept'] })
  const result = await mutateLibrary('failure', async () => saved, async (value) => ({ library: { templates: [...value.templates, 'retry'] } }), async (value) => { saved = value })
  assert.deepEqual(result.library, { templates: ['kept', 'retry'] })
})

test('unreadable and invalid stored values are never replaced by an empty fallback', async () => {
  let writes = 0
  const failure = new Error('invalid stored library')
  await assert.rejects(loadLibrary('invalid', async () => ({ broken: true }), async () => { throw failure }), failure)
  await assert.rejects(mutateLibrary('invalid', async () => { throw failure }, async () => { assert.fail('must not mutate') }, async () => { writes++ }), failure)
  assert.equal(writes, 0)
})

test('a rejected Rust mutation on corrupt saved data never reaches persistence', async () => {
  let writes = 0
  const corrupt = { version: 999, templates: 'broken' }
  const failure = new Error('unsupported library version')
  await assert.rejects(mutateLibrary('corrupt-mutation', async () => corrupt, async (value) => {
    assert.equal(value, corrupt)
    throw failure
  }, async () => { writes++ }), failure)
  assert.equal(writes, 0)
})

test('concurrent mutations read in order and preserve both changes', async () => {
  let saved = { templates: [] }
  const add = (name) => mutateLibrary('concurrent', async () => saved, async (value) => ({ library: { templates: [...value.templates, name] } }), async (value) => { saved = value })
  await Promise.all([add('first'), add('second')])
  assert.deepEqual(saved, { templates: ['first', 'second'] })
})
