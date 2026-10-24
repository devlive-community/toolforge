import assert from 'node:assert/strict'
import test from 'node:test'
import { setImmediate as tick } from 'node:timers/promises'
import { createInstallQueue } from '../src/lib/installQueue.ts'

const deferred = () => {
  let resolve
  const promise = new Promise((r) => (resolve = r))
  return { promise, resolve }
}

test('runs at most `limit` tasks at once and starts queued tasks in order', async () => {
  let limit = 2
  const queue = createInstallQueue(() => limit)
  const gates = [deferred(), deferred(), deferred(), deferred()]
  const started = []
  const running = []
  let peak = 0

  const jobs = gates.map((gate, i) =>
    queue.run(async () => {
      started.push(i)
      running.push(i)
      peak = Math.max(peak, running.length)
      await gate.promise
      running.splice(running.indexOf(i), 1)
      return i
    }),
  )

  await tick()
  assert.deepEqual(started, [0, 1])

  gates[0].resolve()
  await tick()
  assert.deepEqual(started, [0, 1, 2])

  gates[1].resolve()
  gates[2].resolve()
  await tick()
  gates[3].resolve()
  assert.deepEqual(await Promise.all(jobs), [0, 1, 2, 3])
  assert.equal(peak, 2)
})

test('a rejected task frees its slot and does not block the queue', async () => {
  const queue = createInstallQueue(() => 1)
  const failing = queue.run(() => Promise.reject(new Error('boom')))
  const ok = queue.run(async () => 'done')
  await assert.rejects(failing, /boom/)
  assert.equal(await ok, 'done')
})

test('limit is read at scheduling time, so raising it takes effect for waiting tasks', async () => {
  let limit = 1
  const queue = createInstallQueue(() => limit)
  const first = deferred()
  const started = []
  const a = queue.run(async () => { started.push('a'); await first.promise })
  const b = queue.run(async () => { started.push('b') })
  const c = queue.run(async () => { started.push('c') })
  await tick()
  assert.deepEqual(started, ['a'])
  limit = 3
  first.resolve()
  await Promise.all([a, b, c])
  assert.deepEqual(started, ['a', 'b', 'c'])
})
