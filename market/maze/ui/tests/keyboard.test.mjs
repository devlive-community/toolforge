import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import ts from 'typescript'
import { URL } from 'node:url'
import { Buffer } from 'node:buffer'
const source = readFileSync(new URL('../src/keyboard.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } })
const { mazeKey } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`)
const event = (key, patch = {}) => ({ key, repeat: false, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, defaultPrevented: false, ...patch })
test('maps four directions and consumes a held key without repeated moves', () => {
  assert.deepEqual(
    ['ArrowUp', 'ArrowRight', 'ArrowDown', 'ArrowLeft'].map((key) => mazeKey(event(key))),
    ['top', 'right', 'bottom', 'left'],
  )
  const sequence = [mazeKey(event('ArrowRight')), ...Array.from({ length: 100 }, () => mazeKey(event('ArrowRight', { repeat: true })))]
  assert.equal(sequence.filter((action) => action === 'right').length, 1)
  assert.equal(sequence.filter((action) => action === 'consume').length, 100)
})
test('ignores interactive fields, modifiers and already handled events', () => {
  assert.equal(mazeKey(event('ArrowUp'), true), null)
  for (const flag of ['altKey', 'ctrlKey', 'metaKey', 'shiftKey', 'defaultPrevented']) assert.equal(mazeKey(event('ArrowUp', { [flag]: true })), null)
})
test('unrelated keys remain available to the browser', () => {
  for (const key of ['Tab', ' ', 'Enter', 'Escape', 'a']) assert.equal(mazeKey(event(key)), null)
})
