import type { CSSProperties } from 'react'

/** 0–1 的热度：按平方根压缩，少数高频键不会让其他键都显得很淡 */
export function heat(count: number, max: number) {
  return max > 0 && count > 0 ? Math.sqrt(count / max) : 0
}

/** 暖色渐变：同色系相邻插值，中间不会出现混浊的过渡色 */
const STOPS = ['--color-warning-soft', '--color-warning', '--color-danger']

/** 热度对应的颜色，在语义色之间插值 */
export function heatColor(level: number) {
  const scaled = Math.min(Math.max(level, 0), 1) * (STOPS.length - 1)
  const index = Math.min(Math.floor(scaled), STOPS.length - 2)
  const mix = Math.round((scaled - index) * 100)
  return `color-mix(in oklab, var(${STOPS[index + 1]}) ${mix}%, var(${STOPS[index]}))`
}

/** 键帽与格子的样式：没有按过的键保持底色 */
export function heatStyle(level: number): CSSProperties {
  if (level <= 0) return {}
  return { backgroundColor: heatColor(level) }
}

/** 颜色较深时文字改为反色 */
export const heatText = (level: number) => (level > 0.4 ? 'text-fg-on-primary' : 'text-fg')

/** 图例渐变 */
export const legendGradient = `linear-gradient(to right, ${[0, 0.5, 1].map(heatColor).join(', ')})`
