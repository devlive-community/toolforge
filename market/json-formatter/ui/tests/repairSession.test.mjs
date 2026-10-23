import assert from 'node:assert/strict'
import test from 'node:test'
import { createRepairSession, matchesRepairSnapshot } from '../src/repairSession.ts'

const source = { input: "{name: 'ToolForge'}", indent: '2', sortKeys: false }
const errorOf = () => ({ code: 'json.repair_syntax', params: { line: 1, column: 1 } })

for (const change of ['edit input', 'change indent', 'toggle sorting', 'switch tab and back']) {
  test(`a late repair cannot publish after ${change}, even if the original snapshot returns`, async () => {
    const states = []
    const waiting = Promise.withResolvers()
    const session = createRepairSession((state) => states.push(state))
    const pending = session.start(source, () => waiting.promise, errorOf)
    const oldSequence = states.at(-1).sequence
    session.invalidate()
    waiting.resolve({ output: '{"name":"ToolForge"}' })
    await pending
    assert.equal(states.at(-1).status, 'idle')
    assert.equal(session.isCurrent(oldSequence), false)
    assert.equal(states.some((state) => state.status === 'success'), false)
  })
}

test('out-of-order completions preserve only the newest explicit submission', async () => {
  const states = []
  const old = Promise.withResolvers()
  const latest = Promise.withResolvers()
  const session = createRepairSession((state) => states.push(state))
  const first = session.start(source, () => old.promise, errorOf)
  const secondSnapshot = { ...source, input: '{new: True}', indent: '4', sortKeys: true }
  const second = session.start(secondSnapshot, () => latest.promise, errorOf)
  latest.resolve({ output: '{"new":true}' })
  await second
  const current = states.at(-1)
  old.resolve({ output: '{"name":"ToolForge"}' })
  await first
  assert.equal(states.at(-1), current)
  assert.equal(current.result.output, '{"new":true}')
  assert.equal(matchesRepairSnapshot(current.snapshot, secondSnapshot), true)
  assert.equal(matchesRepairSnapshot(current.snapshot, source), false)
})

test('unmount discards success and failures without publishing or retaining an applicable sequence', async () => {
  for (const rejects of [false, true]) {
    const states = []
    const waiting = Promise.withResolvers()
    const session = createRepairSession((state) => states.push(state))
    const pending = session.start(source, () => waiting.promise, errorOf)
    const sequence = states.at(-1).sequence
    session.close()
    if (rejects) waiting.reject(new Error('late error'))
    else waiting.resolve({ output: '{}' })
    await pending
    assert.equal(states.length, 1)
    assert.equal(session.isCurrent(sequence), false)
  }
})

test('strict-mode reactivation accepts a fresh run while an earlier lifetime remains revoked', async () => {
  const states = []
  const previous = Promise.withResolvers()
  const session = createRepairSession((state) => states.push(state))
  const first = session.start(source, () => previous.promise, errorOf)
  session.close()
  session.activate()
  await session.start(source, async () => ({ output: '{"fresh":true}' }), errorOf)
  const current = states.at(-1)
  previous.resolve({ output: '{"old":true}' })
  await first
  assert.equal(states.at(-1), current)
  assert.equal(current.result.output, '{"fresh":true}')
})

test('a stale failure cannot replace a newer result; a current failure keeps the submitted snapshot', async () => {
  const states = []
  const old = Promise.withResolvers()
  const session = createRepairSession((state) => states.push(state))
  const first = session.start(source, () => old.promise, errorOf)
  await session.start(source, async () => ({ output: '{}' }), errorOf)
  old.reject(new Error('old failure'))
  await first
  assert.equal(states.at(-1).status, 'success')
  await session.start(source, async () => { throw new Error('current failure') }, errorOf)
  assert.deepEqual(states.at(-1).error, errorOf())
  assert.deepEqual(states.at(-1).snapshot, source)
})

test('the submitted options are copied and every option participates in preview freshness', async () => {
  const states = []
  const waiting = Promise.withResolvers()
  const snapshot = { ...source }
  let received
  const session = createRepairSession((state) => states.push(state))
  const pending = session.start(snapshot, (args) => { received = args; return waiting.promise }, errorOf)
  snapshot.input = 'edited'
  snapshot.indent = 'tab'
  snapshot.sortKeys = true
  waiting.resolve({ output: '{}' })
  await pending
  assert.deepEqual(received, source)
  assert.equal(matchesRepairSnapshot(source, snapshot), false)
  for (const changed of [{ ...source, input: 'changed' }, { ...source, indent: '4' }, { ...source, sortKeys: true }]) {
    assert.equal(matchesRepairSnapshot(source, changed), false)
  }
})
