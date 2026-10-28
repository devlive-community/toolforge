import assert from 'node:assert/strict'
import test from 'node:test'
import { localDate, newOpId } from '../src/today.ts'

test('local date uses the local calendar day with zero padding', () => {
  assert.equal(localDate(new Date(2026, 0, 5, 23, 59)), '2026-01-05')
  assert.equal(localDate(new Date(2026, 11, 31, 0, 1)), '2026-12-31')
})

test('operation ids are unique', () => {
  const ids = new Set(Array.from({ length: 50 }, () => newOpId()))
  assert.equal(ids.size, 50)
})
