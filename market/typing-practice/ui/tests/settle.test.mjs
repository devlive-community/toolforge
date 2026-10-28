import assert from 'node:assert/strict'
import test from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'
import { createQueue, createSettler, mergeMistakes, pasteFlag } from '../src/settle.ts'

const outcome = (overrides = {}) => ({ cpm: 120, pasted: false, activeMs: 1000, ...overrides })

test('a round settles exactly once even if completion arrives twice', () => {
  const settler = createSettler()
  const settled = []
  settler.complete(outcome(), (r) => settled.push(r))
  settler.complete(outcome(), (r) => settled.push(r))
  assert.equal(settled.length, 1)
})

test('reset reopens settlement for the next round', () => {
  const settler = createSettler()
  const settled = []
  settler.complete(outcome({ cpm: 100 }), (r) => settled.push(r))
  settler.reset()
  settler.complete(outcome({ cpm: 90 }), (r) => settled.push(r))
  assert.equal(settled.length, 2)
})

test('pasted rounds are reported as practice and never stored as records', () => {
  const settler = createSettler()
  const settled = []
  settler.complete(outcome({ cpm: 200, pasted: true }), (r) => settled.push(r))
  assert.equal(settled[0].record, false)
})

test('clean rounds are stored as records', () => {
  const settler = createSettler()
  const settled = []
  settler.complete(outcome({ cpm: 200, pasted: false }), (r) => settled.push(r))
  assert.equal(settled[0].record, true)
})

test('paste flag is sticky within a round once it has happened', () => {
  assert.equal(pasteFlag(false, true), true)
  assert.equal(pasteFlag(true, false), true)
  assert.equal(pasteFlag(false, false), false)
})

test('the full score payload is passed through unchanged', () => {
  const settler = createSettler()
  const settled = []
  settler.complete({ cpm: 150, wpm: 30, accuracy: 0.97, activeMs: 4200, pasted: false }, (r) => settled.push(r))
  assert.equal(settled[0].wpm, 30)
  assert.equal(settled[0].activeMs, 4200)
})

test('mistakes accumulate per character and keep only the most frequent kinds', () => {
  const merged = mergeMistakes({ a: 2 }, ['a', 'b', 'a'])
  assert.deepEqual(merged, { a: 4, b: 1 })
  const capped = mergeMistakes({ a: 5, b: 1 }, ['c', 'c'], 2)
  assert.deepEqual(capped, { a: 5, c: 2 })
})

test('queued submissions run strictly in order even when an earlier one is slower', async () => {
  const queue = createQueue()
  const order = []
  queue.push(async () => { await sleep(20); order.push(1) })
  queue.push(async () => { order.push(2) })
  await queue.push(async () => { order.push(3) })
  assert.deepEqual(order, [1, 2, 3])
})

test('a failed submission does not block the next one', async () => {
  const queue = createQueue()
  const order = []
  queue.push(async () => { throw new Error('boom') }).catch(() => undefined)
  await queue.push(async () => { order.push('next') })
  assert.deepEqual(order, ['next'])
})
