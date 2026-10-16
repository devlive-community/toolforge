import assert from 'node:assert/strict'
import test from 'node:test'
import { createUpdateActions } from '../src/stores/updateActions.ts'

const INFO = { version: '0.2.1', currentVersion: '0.2.0', date: null, notes: [] }

function harness({ status = 'available', info = INFO, skippedVersion = null } = {}) {
  let state = { status, info, downloaded: 0, total: null, error: null, dialogOpen: true, checkedAt: null }
  const checks = []
  const installs = []
  const actions = createUpdateActions({
    get: () => state,
    set: (patch) => { state = { ...state, ...patch } },
    check: () => {
      const pending = Promise.withResolvers()
      checks.push(pending)
      return pending.promise
    },
    install: (emit) => {
      const pending = { ...Promise.withResolvers(), emit }
      installs.push(pending)
      return pending.promise
    },
    skippedVersion: () => skippedVersion,
    toError: (error) => error,
    now: () => 123,
  })
  return { actions, checks, installs, get state() { return state } }
}

test('an installation failure retains the candidate and retry resets progress and error', async () => {
  const h = harness()
  const failed = h.actions.install()
  h.installs[0].emit({ type: 'progress', downloaded: 500, total: 1000 })
  h.installs[0].emit({ type: 'installing' })
  h.installs[0].reject({ code: 'update.install', params: { detail: 'fixture failure' } })
  await failed
  assert.equal(h.state.status, 'error')
  assert.equal(h.state.info, INFO)
  assert.equal(h.state.error.code, 'update.install')

  const retry = h.actions.install()
  assert.equal(h.installs.length, 2)
  assert.equal(h.state.status, 'downloading')
  assert.equal(h.state.downloaded, 0)
  assert.equal(h.state.total, null)
  assert.equal(h.state.error, null)
  assert.equal(h.state.info, INFO)
  h.installs[1].resolve()
  await retry
  assert.equal(h.state.status, 'ready', 'the command result completes the UI even without a finished event')
})

test('late messages from a failed attempt cannot overwrite its error or the next attempt', async () => {
  const h = harness()
  const first = h.actions.install()
  const old = h.installs[0]
  old.reject({ code: 'update.download' })
  await first
  const failedState = h.state
  old.emit({ type: 'finished' })
  old.emit({ type: 'installing' })
  old.emit({ type: 'progress', downloaded: 999, total: 999 })
  assert.deepEqual(h.state, failedState)

  const retry = h.actions.install()
  h.installs[1].emit({ type: 'progress', downloaded: 20, total: 200 })
  const retryState = h.state
  old.emit({ type: 'finished' })
  old.emit({ type: 'installing' })
  old.emit({ type: 'progress', downloaded: 999, total: 999 })
  assert.deepEqual(h.state, retryState)
  h.installs[1].resolve()
  await retry
})

test('late messages cannot regress a successful installation waiting for restart', async () => {
  const h = harness()
  const installing = h.actions.install()
  h.installs[0].emit({ type: 'progress', downloaded: 100, total: 100 })
  h.installs[0].resolve()
  await installing
  const ready = h.state
  h.installs[0].emit({ type: 'progress', downloaded: 1, total: null })
  h.installs[0].emit({ type: 'installing' })
  await h.actions.install()
  await h.actions.check()
  assert.deepEqual(h.state, ready)
  assert.equal(h.installs.length, 1)
  assert.equal(h.checks.length, 0)
})

test('a finished event cannot permit a duplicate install before the command settles', async () => {
  const h = harness()
  const installing = h.actions.install()
  h.installs[0].emit({ type: 'finished' })
  assert.equal(h.state.status, 'ready')
  h.installs[0].emit({ type: 'installing' })
  await h.actions.install()
  await h.actions.check()
  assert.equal(h.installs.length, 1)
  assert.equal(h.checks.length, 0)
  assert.equal(h.state.status, 'ready')
  h.installs[0].resolve()
  await installing
})

test('an IPC rejection after a finished event preserves the installed state', async () => {
  const h = harness()
  const installing = h.actions.install()
  h.installs[0].emit({ type: 'finished' })
  h.installs[0].reject({ code: 'app.unknown', params: { detail: 'fixture IPC response lost' } })
  await installing
  assert.equal(h.state.status, 'ready')
  assert.equal(h.state.error, null)
  h.installs[0].emit({ type: 'installing' })
  await h.actions.install()
  assert.equal(h.state.status, 'ready')
  assert.equal(h.installs.length, 1)
})

test('check, download, install, and ready states block competing commands', async () => {
  for (const status of ['checking', 'downloading', 'installing', 'ready']) {
    const h = harness({ status })
    await Promise.all([h.actions.install(), h.actions.check()])
    assert.equal(h.state.status, status)
    assert.equal(h.installs.length, 0)
    assert.equal(h.checks.length, 0)
  }
  const h = harness({ status: 'error', info: null })
  await h.actions.install()
  assert.equal(h.state.status, 'error')
  assert.equal(h.installs.length, 0)
})

test('rapid repeated clicks launch only one operation and checking blocks installation', async () => {
  const h = harness()
  const checking = h.actions.check()
  await h.actions.check()
  await h.actions.install()
  assert.equal(h.checks.length, 1)
  assert.equal(h.installs.length, 0)
  h.checks[0].resolve(INFO)
  await checking
  const installing = h.actions.install()
  await h.actions.install()
  await h.actions.check()
  assert.equal(h.checks.length, 1)
  assert.equal(h.installs.length, 1)
  h.installs[0].resolve()
  await installing
})

test('silent checks still honor skipped versions while manual checks show the dialog', async () => {
  for (const silent of [true, false]) {
    const h = harness({ status: 'idle', info: null, skippedVersion: INFO.version })
    const checking = h.actions.check({ silent })
    h.checks[0].resolve(INFO)
    await checking
    assert.equal(h.state.status, 'available')
    assert.equal(h.state.info, INFO)
    assert.equal(h.state.dialogOpen, !silent)
    assert.equal(h.state.checkedAt, 123)
  }
})

test('latest and failed checks keep the existing silent/manual behavior', async () => {
  const latest = harness()
  const checking = latest.actions.check()
  latest.checks[0].resolve(null)
  await checking
  assert.equal(latest.state.status, 'latest')
  assert.equal(latest.state.info, null)
  assert.equal(latest.state.dialogOpen, false)
  for (const silent of [true, false]) {
    const h = harness({ status: 'idle', info: null })
    const failed = h.actions.check({ silent })
    h.checks[0].reject({ code: 'update.check' })
    await failed
    assert.equal(h.state.status, silent ? 'idle' : 'error')
    assert.equal(h.state.error.code, 'update.check')
    assert.equal(h.state.checkedAt, 123)
  }
})
