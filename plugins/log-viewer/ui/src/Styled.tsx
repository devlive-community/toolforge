import type { CSSProperties, ReactNode } from 'react'
import { cn } from '@toolforge/ui'
import type { AnsiColor, Span } from './types'

/** 终端基本色映射到主题色，深浅主题下都保持可读 */
const FG = ['text-fg-muted', 'text-danger', 'text-success', 'text-warning', 'text-info', 'text-syntax-key', 'text-syntax-bool', 'text-fg']
const BG = ['bg-surface-2', 'bg-danger-soft', 'bg-success-soft', 'bg-warning-soft', 'bg-info-soft', 'bg-primary-soft', 'bg-info-soft', 'bg-surface-2']

function colorClass(color: AnsiColor | undefined, table: string[]) {
  return typeof color === 'number' ? table[color % 8] : undefined
}

/** 256 色与 truecolor 来自日志内容本身，只能以内联样式呈现 */
function colorStyle(span: Span): CSSProperties | undefined {
  const fg = typeof span.fg === 'string' ? span.fg : undefined
  const bg = typeof span.bg === 'string' ? span.bg : undefined
  if (!fg && !bg) return undefined
  return span.inverse ? { color: bg, backgroundColor: fg } : { color: fg, backgroundColor: bg }
}

function spanClass(span: Span) {
  const [fg, bg] = span.inverse ? [colorClass(span.bg, FG), colorClass(span.fg, BG)] : [colorClass(span.fg, FG), colorClass(span.bg, BG)]
  return cn(
    fg,
    bg ?? (span.inverse && 'bg-fg text-bg'),
    typeof span.fg === 'number' && span.fg >= 8 && 'font-semibold',
    span.bold && 'font-semibold',
    span.dim && 'opacity-70',
    span.italic && 'italic',
    span.underline && 'underline',
    span.strike && 'line-through',
  )
}

/**
 * 按样式区间与匹配高亮渲染一行文本（位置均由 Rust 按 UTF-16 下标给出）。
 * colors 为 false 时忽略样式，只保留高亮。
 */
export function Styled({ text, styles, marks = [], colors }: { text: string; styles: Span[]; marks?: [number, number][]; colors: boolean }) {
  const spans = colors ? styles : []
  if (spans.length === 0 && marks.length === 0) return <>{text}</>
  const cuts = new Set<number>([0, text.length])
  for (const s of spans) {
    cuts.add(s.start)
    cuts.add(s.end)
  }
  for (const [start, end] of marks) {
    cuts.add(start)
    cuts.add(end)
  }
  const points = [...cuts].filter((p) => p >= 0 && p <= text.length).sort((a, b) => a - b)
  const parts: ReactNode[] = []
  for (let i = 0; i < points.length - 1; i++) {
    const [from, to] = [points[i], points[i + 1]]
    if (from === to) continue
    const span = spans.find((s) => s.start <= from && to <= s.end)
    const marked = marks.some(([start, end]) => start <= from && to <= end)
    const piece = text.slice(from, to)
    const content = span ? (
      <span className={spanClass(span)} style={colorStyle(span)}>
        {piece}
      </span>
    ) : (
      piece
    )
    parts.push(
      marked ? (
        <mark key={from} className="rounded-[2px] bg-warning-soft text-fg">
          {content}
        </mark>
      ) : (
        <span key={from}>{content}</span>
      ),
    )
  }
  return <>{parts}</>
}
