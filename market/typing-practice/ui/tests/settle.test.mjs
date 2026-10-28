import assert from 'node:assert/strict'
import test from 'node:test'
import { createSettler, pasteFlag } from '../src/settle.ts'

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
