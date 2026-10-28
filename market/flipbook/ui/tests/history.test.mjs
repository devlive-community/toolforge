import assert from 'node:assert/strict'
import test from 'node:test'
import { setTimeout as sleep } from 'node:timers/promises'
import { MAX_HISTORY, createQueue, dropIndex, emptyHistory, record, redo, referenced, undo } from '../src/history.ts'

test('undo restores the previous frame order and redo brings it back', () => {
  let history = record(emptyHistory(), ['f1'])
  const undone = undo(history, ['f1', 'f2'])
  assert.deepEqual(undone.frames, ['f1'])
  history = undone.history
  const redone = redo(history, ['f1'])
  assert.deepEqual(redone.frames, ['f1', 'f2'])
})

test('undo can bring back a deleted frame after switching frames', () => {
  // 删除前的顺序被记录；切换帧不改变历史，撤销后被删的帧回来
  const history = record(emptyHistory(), ['f1', 'f2', 'f3'])
  assert.deepEqual(undo(history, ['f1', 'f3']).frames, ['f1', 'f2', 'f3'])
})

test('a new change clears the redo stack', () => {
  let history = record(emptyHistory(), ['a'])
  history = undo(history, ['a', 'b']).history
  history = record(history, ['a'])
  assert.equal(history.future.length, 0)
  assert.equal(redo(history, ['a', 'c']), null)
})

test('history depth is capped by dropping the oldest steps', () => {
  let history = emptyHistory()
  for (let i = 0; i < MAX_HISTORY + 5; i += 1) history = record(history, [`f${i}`])
  assert.equal(history.past.length, MAX_HISTORY)
  assert.deepEqual(history.past[0], ['f5'])
})

test('nothing to undo returns null', () => {
  assert.equal(undo(emptyHistory(), ['f1']), null)
})

test('drop index accounts for the dragged frame leaving its slot', () => {
  // [a b c d]：把 a 拖到 c 后面 → 新位置 2
  assert.equal(dropIndex(0, 2, 'after'), 2)
  // 把 d 拖到 a 前面 → 0
  assert.equal(dropIndex(3, 0, 'before'), 0)
  // 放回自己旁边不移动
  assert.equal(dropIndex(1, 1, 'before'), 1)
  assert.equal(dropIndex(1, 0, 'after'), 1)
})

test('queued edits run in order even if an earlier one is slower or fails', async () => {
  const queue = createQueue()
  const order = []
  queue.push(async () => { await sleep(15); order.push(1) })
  queue.push(async () => { throw new Error('boom') }).catch(() => undefined)
  await queue.push(async () => { order.push(3) })
  assert.deepEqual(order, [1, 3])
})

test('frames still reachable by undo or redo are kept when collecting', () => {
  let history = record(emptyHistory(), ['f1'])
  history = record(history, ['f2'])
  history = undo(history, ['f3']).history
  assert.deepEqual(referenced(history).sort(), ['f1', 'f3'])
})
