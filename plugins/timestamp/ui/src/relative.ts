/** 相对时间的展示格式化（数值由 Rust 计算） */
const STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60],
  ['second', 1],
]

export function formatRelative(seconds: number, locale: string) {
  const format = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
  const [unit, size] = STEPS.find(([, size]) => Math.abs(seconds) >= size) ?? ['second', 1]
  return format.format(Math.round(seconds / size), unit)
}
