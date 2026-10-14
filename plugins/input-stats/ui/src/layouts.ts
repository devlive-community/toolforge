import type { Platform } from './types'

/** 键盘上的一个键，坐标与尺寸以标准键宽（1u）为单位 */
export interface KeyRect {
  code: string
  x: number
  y: number
  w: number
  h: number
}

export interface KeyboardLayout {
  width: number
  height: number
  keys: KeyRect[]
}

type Item = [code: string | null, w?: number]

/** 从 x0 开始横向排列一行；code 为 null 表示空隙 */
function row(keys: KeyRect[], y: number, x0: number, items: Item[], h = 1) {
  let x = x0
  for (const [code, w = 1] of items) {
    if (code) keys.push({ code, x, y, w, h })
    x += w
  }
}

const letters = (s: string): Item[] => [...s].map((c) => [`Key${c}`])
const digits: Item[] = [...'1234567890'].map((d) => [`Digit${d}`])
const fkeys = (from: number, to: number, w = 1): Item[] =>
  Array.from({ length: to - from + 1 }, (_, i) => [`F${from + i}`, w])

/** 主键区第 1–4 行（数字行到 Shift 行），15u 宽 */
function mainBlock(keys: KeyRect[], y: number) {
  row(keys, y, 0, [['Backquote'], ...digits, ['Minus'], ['Equal'], ['Backspace', 2]])
  row(keys, y + 1, 0, [['Tab', 1.5], ...letters('QWERTYUIOP'), ['BracketLeft'], ['BracketRight'], ['Backslash', 1.5]])
  row(keys, y + 2, 0, [['CapsLock', 1.75], ...letters('ASDFGHJKL'), ['Semicolon'], ['Quote'], ['Enter', 2.25]])
  row(keys, y + 3, 0, [['ShiftLeft', 2.25], ...letters('ZXCVBNM'), ['Comma'], ['Period'], ['Slash'], ['ShiftRight', 2.75]])
}

/** 笔记本键盘：短功能键行 + 主键区 + 倒 T 方向键 */
function compact(platform: Platform): KeyboardLayout {
  const keys: KeyRect[] = []
  const fn = 0.7
  row(keys, 0, 0, [['Escape', 1.5], ...fkeys(1, 12, 13.5 / 12)], fn)
  const y = fn + 0.1
  mainBlock(keys, y)
  const bottom: Item[] =
    platform === 'mac'
      ? [['Fn'], ['ControlLeft'], ['AltLeft'], ['MetaLeft', 1.25], ['Space', 5.5], ['MetaRight', 1.25], ['AltRight']]
      : [['ControlLeft', 1.25], ['Fn'], ['MetaLeft', 1.25], ['AltLeft', 1.25], ['Space', 5], ['AltRight'], ['ControlRight', 1.25]]
  row(keys, y + 4, 0, [...bottom, ['ArrowLeft']])
  keys.push({ code: 'ArrowUp', x: 13, y: y + 4, w: 1, h: 0.5 })
  keys.push({ code: 'ArrowDown', x: 13, y: y + 4.5, w: 1, h: 0.5 })
  row(keys, y + 4, 14, [['ArrowRight']])
  return { width: 15, height: y + 5, keys }
}

/** 全尺寸键盘：功能键行 + 主键区 + 编辑区 + 数字小键盘 */
function full(platform: Platform): KeyboardLayout {
  const keys: KeyRect[] = []
  const mac = platform === 'mac'
  row(keys, 0, 0, [['Escape'], [null], ...fkeys(1, 4), [null, 0.5], ...fkeys(5, 8), [null, 0.5], ...fkeys(9, 12)])
  row(keys, 0, 15.5, mac ? fkeys(13, 15) : [['PrintScreen'], ['ScrollLock'], ['Pause']])
  if (mac) row(keys, 0, 19, fkeys(16, 19))
  const y = 1.5
  mainBlock(keys, y)
  row(
    keys,
    y + 4,
    0,
    mac
      ? [['ControlLeft', 1.5], ['AltLeft', 1.25], ['MetaLeft', 1.5], ['Space', 6.5], ['MetaRight', 1.5], ['AltRight', 1.25], ['ControlRight', 1.5]]
      : [['ControlLeft', 1.25], ['MetaLeft', 1.25], ['AltLeft', 1.25], ['Space', 6.25], ['AltRight', 1.25], ['MetaRight', 1.25], ['ContextMenu', 1.25], ['ControlRight', 1.25]],
  )
  // 编辑区与方向键
  row(keys, y, 15.5, [[mac ? 'Fn' : 'Insert'], ['Home'], ['PageUp']])
  row(keys, y + 1, 15.5, [['Delete'], ['End'], ['PageDown']])
  row(keys, y + 3, 16.5, [['ArrowUp']])
  row(keys, y + 4, 15.5, [['ArrowLeft'], ['ArrowDown'], ['ArrowRight']])
  // 数字小键盘
  const nx = 19
  row(keys, y, nx, mac ? [['NumLock'], ['NumpadEqual'], ['NumpadDivide'], ['NumpadMultiply']] : [['NumLock'], ['NumpadDivide'], ['NumpadMultiply'], ['NumpadSubtract']])
  row(keys, y + 1, nx, [['Numpad7'], ['Numpad8'], ['Numpad9']])
  row(keys, y + 2, nx, [['Numpad4'], ['Numpad5'], ['Numpad6']])
  row(keys, y + 3, nx, [['Numpad1'], ['Numpad2'], ['Numpad3']])
  row(keys, y + 4, nx, [['Numpad0', 2], ['NumpadDecimal']])
  if (mac) {
    row(keys, y + 1, nx + 3, [['NumpadSubtract']])
    row(keys, y + 2, nx + 3, [['NumpadAdd']])
  } else {
    keys.push({ code: 'NumpadAdd', x: nx + 3, y: y + 1, w: 1, h: 2 })
  }
  keys.push({ code: 'NumpadEnter', x: nx + 3, y: y + 3, w: 1, h: 2 })
  return { width: 23, height: y + 5, keys }
}

export type LayoutKind = 'compact' | 'full'

export function keyboardLayout(kind: LayoutKind, platform: Platform): KeyboardLayout {
  return kind === 'compact' ? compact(platform) : full(platform)
}

/** 只在全尺寸键盘上出现的键：统计到这些键时默认显示全尺寸布局 */
export const FULL_ONLY = /^(Numpad|Insert$|Home$|End$|PageUp$|PageDown$|PrintScreen$|ScrollLock$|Pause$|NumLock$|F1[3-9]$)/

const SYMBOLS: Record<string, string> = {
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Comma: ',',
  Period: '.',
  Slash: '/',
  NumpadDivide: '/',
  NumpadMultiply: '*',
  NumpadSubtract: '-',
  NumpadAdd: '+',
  NumpadDecimal: '.',
  NumpadEqual: '=',
  NumpadComma: ',',
  ArrowLeft: '←',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowRight: '→',
  Space: '',
}

const MAC: Record<string, string> = {
  Escape: 'esc',
  Backspace: '⌫',
  Tab: '⇥',
  CapsLock: '⇪',
  Enter: '↩',
  NumpadEnter: '⌤',
  ShiftLeft: '⇧',
  ShiftRight: '⇧',
  ControlLeft: '⌃',
  ControlRight: '⌃',
  AltLeft: '⌥',
  AltRight: '⌥',
  MetaLeft: '⌘',
  MetaRight: '⌘',
  Fn: 'fn',
  Delete: '⌦',
  Home: '↖',
  End: '↘',
  PageUp: '⇞',
  PageDown: '⇟',
  NumLock: '⌧',
}

const PC: Record<string, string> = {
  Escape: 'Esc',
  Backspace: 'Backspace',
  Tab: 'Tab',
  CapsLock: 'Caps',
  Enter: 'Enter',
  NumpadEnter: 'Enter',
  ShiftLeft: 'Shift',
  ShiftRight: 'Shift',
  ControlLeft: 'Ctrl',
  ControlRight: 'Ctrl',
  AltLeft: 'Alt',
  AltRight: 'Alt',
  MetaLeft: 'Win',
  MetaRight: 'Win',
  ContextMenu: 'Menu',
  Fn: 'Fn',
  Insert: 'Ins',
  Delete: 'Del',
  Home: 'Home',
  End: 'End',
  PageUp: 'PgUp',
  PageDown: 'PgDn',
  PrintScreen: 'PrtSc',
  ScrollLock: 'ScrLk',
  Pause: 'Pause',
  NumLock: 'Num',
}

/** 键帽上显示的文字；Linux 的 Meta 键显示为 Super */
export function keyLabel(code: string, platform: Platform, os?: string): string {
  if (code.startsWith('Key') && code.length === 4) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  if (/^Numpad\d$/.test(code)) return code.slice(6)
  if (/^F\d+$/.test(code)) return code
  if (code in SYMBOLS) return SYMBOLS[code]
  if (os === 'linux' && code.startsWith('Meta')) return 'Super'
  const table = platform === 'mac' ? MAC : PC
  return table[code] ?? code
}

const MODIFIERS: [string, string, string][] = [
  ['Ctrl', '⌃', 'Ctrl'],
  ['Alt', '⌥', 'Alt'],
  ['Shift', '⇧', 'Shift'],
  ['Meta', '⌘', 'Win'],
]

/** 组合键名称：macOS 用 ⌃⌥⇧⌘ 符号连写，其他系统用 Ctrl+Shift+S */
export function comboLabel(combo: string, platform: Platform, os?: string): string {
  const parts = combo.split('+')
  const code = parts.pop() ?? ''
  const key = keyLabel(code, platform, os) || 'Space'
  const mods = MODIFIERS.filter(([name]) => parts.includes(name)).map(([name, mac, pc]) =>
    platform === 'mac' ? mac : name === 'Meta' && os === 'linux' ? 'Super' : pc,
  )
  return platform === 'mac' ? `${mods.join('')}${key.length === 1 ? key.toUpperCase() : key}` : [...mods, key].join('+')
}
