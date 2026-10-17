import assert from 'node:assert/strict'
import test from 'node:test'
import { setImmediate as nextTurn } from 'node:timers/promises'
import { createPersistenceCoordinator, createWorkspacePersistence } from '../src/workspacePersistence.ts'

for (const operationName of ['create request', 'duplicate request']) {
  test(`remount waits for an existing writer, pending ${operationName}, and its final snapshot`, { timeout: 1000 }, async () => {
    const coordinator = createPersistenceCoordinator()
    const firstWriteStarted = Promise.withResolvers()
    const releaseFirstWrite = Promise.withResolvers()
    const lastWriteStarted = Promise.withResolvers()
    const releaseLastWrite = Promise.withResolvers()
    const newId = Promise.withResolvers()
    let database = { requests: [] }
    let writes = 0
    let workspace = { requests: ['initial'] }
    const oldMount = createWorkspacePersistence(coordinator, 'http', {
      validate: async (value) => ({ requests: [...value.requests] }),
      persist: async (value) => {
        writes++
        if (writes === 1) { firstWriteStarted.resolve(); await releaseFirstWrite.promise }
        else { lastWriteStarted.resolve(); await releaseLastWrite.promise }
        database = value
      },
      onStatus: () => {},
    })

    oldMount.stage(workspace)
    const originalWrite = oldMount.flush()
    await firstWriteStarted.promise
    const mutation = oldMount.run(async () => {
      const id = await newId.promise
      workspace = { requests: [...workspace.requests, id] }
      oldMount.stage(workspace)
    })
    const closed = oldMount.close()
    assert.equal(oldMount.flush(), closed, 'a cleanup must extend the writer instead of returning it')
    let loaded = false
    const remountLoad = coordinator.wait('http').then(() => { loaded = true; return database })

    releaseFirstWrite.resolve()
    await originalWrite
    await nextTurn()
    assert.equal(loaded, false, 'an old writer finishing must not release the new mount')

    newId.resolve(operationName)
    await lastWriteStarted.promise
    assert.equal(loaded, false, 'new_id completing must not release the load before its snapshot is saved')
    releaseLastWrite.resolve()
    assert.deepEqual(await remountLoad, { requests: ['initial', operationName] })
    await Promise.all([closed, mutation])

    const newMount = createWorkspacePersistence(coordinator, 'http', {
      validate: async (value) => value,
      persist: async (value) => { database = value },
      onStatus: () => {},
    })
    newMount.stage({ requests: [...database.requests, 'new mount edit'] })
    await newMount.flush()
    await oldMount.flush() // A leftover debounce must have no stale snapshot to write.
    assert.deepEqual(database.requests, ['initial', operationName, 'new mount edit'])
    assert.equal(writes, 2)
  })
}

test('drops obsolete validation results and only persists sanitized snapshots without changing session secrets', { timeout: 1000 }, async () => {
  const coordinator = createPersistenceCoordinator()
  const validationStarted = Promise.withResolvers()
  const releaseValidation = Promise.withResolvers()
  const saved = []
  const first = { label: 'old', secret: 'old session secret' }
  const latest = { label: 'latest', secret: 'current session secret' }
  let validations = 0
  const queue = createWorkspacePersistence(coordinator, 'http', {
    validate: async (value) => {
      if (++validations === 1) { validationStarted.resolve(); await releaseValidation.promise }
      return { ...value, secret: '' }
    },
    persist: async (value) => { saved.push(value) },
    onStatus: () => {},
  })
  queue.stage(first)
  const flushing = queue.flush()
  await validationStarted.promise
  queue.stage(latest)
  releaseValidation.resolve()
  await flushing
  assert.deepEqual(saved, [{ label: 'latest', secret: '' }])
  assert.equal(latest.secret, 'current session secret')
  assert.equal(first.secret, 'old session secret')
})

test('a failed async mutation cannot deadlock cleanup or discard an already pending save', { timeout: 1000 }, async () => {
  const coordinator = createPersistenceCoordinator()
  const releaseMutation = Promise.withResolvers()
  const saved = []
  const queue = createWorkspacePersistence(coordinator, 'http', {
    validate: async (value) => value,
    persist: async (value) => { saved.push(value) },
    onStatus: () => {},
  })
  queue.stage('existing edit')
  const mutation = queue.run(async () => { await releaseMutation.promise; throw new Error('new_id unavailable') })
  const rejection = assert.rejects(mutation, /new_id unavailable/)
  const closed = queue.close()
  releaseMutation.resolve()
  await Promise.all([closed, rejection, coordinator.wait('http')])
  assert.deepEqual(saved, ['existing edit'])
})

test('write failures stay retryable and cleanup settles without spinning on the failed snapshot', { timeout: 1000 }, async () => {
  const coordinator = createPersistenceCoordinator()
  const statuses = []
  let unavailable = true
  let database = null
  const queue = createWorkspacePersistence(coordinator, 'http', {
    validate: async (value) => value,
    persist: async (value) => {
      if (unavailable) throw new Error('SQLite unavailable')
      database = value
    },
    onStatus: (status) => statuses.push(status),
  })
  queue.stage('unsaved edit')
  await queue.flush()
  assert.equal(statuses.at(-1), 'error')
  await queue.close()
  await coordinator.wait('http')
  assert.equal(database, null)
  unavailable = false
  await queue.flush()
  assert.equal(database, 'unsaved edit')
  assert.equal(statuses.at(-1), 'saved')
})

test('an effect cleanup can settle and resume the same queue during a React strict-mode remount', { timeout: 1000 }, async () => {
  const coordinator = createPersistenceCoordinator()
  const saved = []
  const queue = createWorkspacePersistence(coordinator, 'http', {
    validate: async (value) => value,
    persist: async (value) => { saved.push(value) },
    onStatus: () => {},
  })
  queue.stage('before cleanup')
  await queue.close()
  queue.stage('after strict remount')
  await queue.flush()
  assert.deepEqual(saved, ['before cleanup', 'after strict remount'])
})
