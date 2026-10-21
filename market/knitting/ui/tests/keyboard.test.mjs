import assert from 'node:assert/strict'
import test from 'node:test'
import { counterKeyAction } from '../src/keyboard.ts'

const space = (fields = {}) => ({ code: 'Space', repeat: false, defaultPrevented: false, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...fields })

test('holding space advances once and consumes repeated keydowns', () => {
  const actions = [space(), ...Array.from({ length: 100 }, () => space({ repeat: true }))].map((event) => counterKeyAction(event, false))
  assert.equal(actions.filter((action) => action === 'advance').length, 1)
  assert.equal(actions.filter((action) => action === 'consume').length, 100)
  assert.equal(counterKeyAction(space(), false), 'advance', 'a later physical press may advance again')
})

test('typing and native controls keep their spaces and never advance', () => {
  for (const event of [space(), space({ repeat: true })]) assert.equal(counterKeyAction(event, true), 'ignore')
})

test('modified or already handled spaces and other keys are ignored', () => {
  for (const field of ['ctrlKey', 'metaKey', 'altKey', 'shiftKey', 'defaultPrevented']) assert.equal(counterKeyAction(space({ [field]: true }), false), 'ignore')
  assert.equal(counterKeyAction(space({ code: 'Enter' }), false), 'ignore')
})
