import { useLayoutEffect, useRef, useState, type Ref } from 'react'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { PALETTE, type Plot, type Scale, type Series, type YAxis } from './types'

const FONT = 11
const LEGEND_ROW = 20

type Map = (value: number) => number

function linearMap(scale: Scale, from: number, to: number): Map {
  const span = scale.max - scale.min || 1
  return (v) => from + ((v - scale.min) / span) * (to - from)
}

function logMap(scale: Scale, from: number, to: number): Map {
  const lo = Math.log10(scale.min)
  const span = Math.log10(scale.max) - lo || 1
  return (v) => from + ((Math.log10(v) - lo) / span) * (to - from)
}

function yMap(axis: YAxis, top: number, bottom: number): Map {
  return axis.log ? logMap(axis, bottom, top) : linearMap(axis, bottom, top)
}

/** 数值的简短显示：最多 6 位有效数字 */
export function formatValue(value: number) {
  return Number.isInteger(value) ? value.toLocaleString() : value.toLocaleString(undefined, { maximumSignificantDigits: 6 })
}

/** 近似文字宽度：全角字符按一个字号，其余按 0.62 个字号 */
function textWidth(text: string) {
  let width = 0
  for (const char of text) width += (char.codePointAt(0) ?? 0) > 0x2e80 ? FONT : FONT * 0.62
  return width
}

/** 图例按近似字宽排成多行 */
function legendLayout(series: Series[], width: number, label: (s: Series) => string) {
  const items: { x: number; y: number; series: Series; color: string; text: string }[] = []
  let x = 0
  let y = 0
  series.forEach((s, i) => {
    const text = label(s)
    const w = 18 + textWidth(text) + 16
    if (x > 0 && x + w > width) {
      x = 0
      y += LEGEND_ROW
    }
    items.push({ x, y, series: s, color: PALETTE[i % PALETTE.length], text })
    x += w
  })
  return { items, height: series.length ? y + LEGEND_ROW : 0 }
}

function truncate(text: string, max: number) {
  return text.length > max ? `${text.slice(0, max - 1)}…` : text
}

interface Hover {
  px: number
  index: number
  rows: { name: string; color: string; value: number }[]
  label: string
}

export function ChartView({ plot, svgRef }: { plot: Plot; svgRef: Ref<SVGSVGElement> }) {
  const { t } = usePlugin()
  const box = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  const [hover, setHover] = useState<Hover | null>(null)

  useLayoutEffect(() => {
    const element = box.current
    if (!element) return
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect
      setSize({ width: Math.floor(width), height: Math.floor(height) })
    })
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  const { width, height } = size
  const sides = plot.left && plot.right
  const legend = legendLayout(plot.series, Math.max(0, width - 32), (s) => `${truncate(s.name, 32)}${sides ? ` (${s.axis === 'left' ? t('chart.leftShort') : t('chart.rightShort')})` : ''}`)
  const margin = {
    left: plot.left ? 64 : 24,
    right: plot.right ? 64 : 24,
    top: 16 + legend.height + 8,
    bottom: plot.x.name ? 52 : 36,
  }
  const plotW = Math.max(10, width - margin.left - margin.right)
  const plotH = Math.max(10, height - margin.top - margin.bottom)
  const left = margin.left
  const right = margin.left + plotW
  const top = margin.top
  const bottom = margin.top + plotH
  const category = plot.x.kind === 'category'
  const xMap = linearMap(plot.x, left, right)
  const yMaps = { left: plot.left ? yMap(plot.left, top, bottom) : null, right: plot.right ? yMap(plot.right, top, bottom) : null }
  const categories = plot.categories ?? []
  const band = category ? plotW / Math.max(1, categories.length) : 0
  const bars = plot.series.length
  // 标签放不下时按间隔显示（刻度线仍全部画出）
  const xLabelWidth = Math.max(0, ...plot.x.ticks.map((tick) => textWidth(truncate(tick.label, 16)))) + 12
  const xEvery = Math.max(1, Math.ceil((plot.x.ticks.length * xLabelWidth) / plotW))

  const xLabel = (value: number) => {
    if (category) return categories[Math.round(value)] ?? ''
    if (plot.x.kind === 'time') return new Date(value).toLocaleString()
    return formatValue(value)
  }

  const onMove = (event: React.MouseEvent<SVGRectElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    const px = event.clientX - rect.left + left
    // 找到离鼠标最近的 x，再取各系列在该 x 上（或最近）的值
    let nearest: number | null = null
    for (const s of plot.series) {
      for (const [x] of s.points) {
        if (nearest === null || Math.abs(xMap(x) - px) < Math.abs(xMap(nearest) - px)) nearest = x
      }
    }
    if (nearest === null) return setHover(null)
    const at = nearest
    const rows = plot.series.flatMap((s, i) => {
      const point = s.points.find((p) => p[0] === at)
      return point ? [{ name: s.name, color: PALETTE[i % PALETTE.length], value: point[1] }] : []
    })
    setHover({ px: xMap(at), index: at, rows, label: xLabel(at) })
  }

  const yTicks = (axis: YAxis | null, side: 'left' | 'right') => {
    const map = side === 'left' ? yMaps.left : yMaps.right
    if (!axis || !map) return null
    const x = side === 'left' ? left - 8 : right + 8
    return (
      <g fontSize={FONT} textAnchor={side === 'left' ? 'end' : 'start'}>
        {axis.ticks.map((tick) => (
          <g key={`${side}-${tick.value}`}>
            {side === 'left' && <line x1={left} x2={right} y1={map(tick.value)} y2={map(tick.value)} stroke="currentColor" strokeOpacity={0.12} />}
            <text x={x} y={map(tick.value)} dy="0.32em" fill="currentColor" fillOpacity={0.7}>
              {tick.label}
            </text>
          </g>
        ))}
        <text
          transform={`translate(${side === 'left' ? 14 : width - 14} ${(top + bottom) / 2}) rotate(${side === 'left' ? -90 : 90})`}
          textAnchor="middle"
          fill="currentColor"
          fillOpacity={0.8}
        >
          {truncate(axis.names.join(' / '), 48)}
        </text>
      </g>
    )
  }

  return (
    <div ref={box} className="relative h-full min-h-0 w-full">
      {width > 0 && height > 0 && (
        <svg ref={svgRef} width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block text-fg" fontFamily="ui-sans-serif, system-ui, sans-serif">
          {/* 图例 */}
          <g transform="translate(16 16)" fontSize={FONT}>
            {legend.items.map((item) => (
              <g key={item.series.column} transform={`translate(${item.x} ${item.y})`}>
                <rect y={2} width={12} height={8} rx={2} fill={item.color} />
                <text x={18} y={6} dy="0.32em" fill="currentColor">
                  {item.text}
                </text>
              </g>
            ))}
          </g>

          {yTicks(plot.left, 'left')}
          {yTicks(plot.right, 'right')}

          {/* X 轴 */}
          <g fontSize={FONT} textAnchor="middle">
            <line x1={left} x2={right} y1={bottom} y2={bottom} stroke="currentColor" strokeOpacity={0.35} />
            {plot.x.ticks.map((tick, i) => {
              const x = category ? left + band * (tick.value + 0.5) : xMap(tick.value)
              return (
                <g key={`x-${tick.value}`}>
                  <line x1={x} x2={x} y1={bottom} y2={bottom + 4} stroke="currentColor" strokeOpacity={0.35} />
                  {i % xEvery === 0 && (
                    <text x={x} y={bottom + 16} fill="currentColor" fillOpacity={0.7}>
                      {truncate(tick.label, 16)}
                    </text>
                  )}
                </g>
              )
            })}
            {plot.x.name && (
              <text x={(left + right) / 2} y={height - 10} fill="currentColor" fillOpacity={0.8}>
                {truncate(plot.x.name, 64)}
              </text>
            )}
          </g>

          {/* 数据 */}
          <g>
            {plot.series.map((s, i) => {
              const map = (s.axis === 'left' ? yMaps.left : yMaps.right) ?? yMaps.left ?? yMaps.right
              if (!map) return null
              const color = PALETTE[i % PALETTE.length]
              if (plot.chart === 'bar') {
                const axis = s.axis === 'left' ? plot.left : plot.right
                const base = map(axis && !axis.log ? Math.max(axis.min, Math.min(axis.max, 0)) : (axis?.min ?? 0))
                const inner = band * 0.8
                const w = Math.max(1, inner / bars)
                return (
                  <g key={s.column} fill={color}>
                    {s.points.map(([x, y]) => {
                      const bx = left + band * x + band * 0.1 + w * i
                      const by = map(y)
                      return <rect key={x} x={bx} y={Math.min(by, base)} width={Math.max(1, w - 1)} height={Math.max(0.5, Math.abs(base - by))} />
                    })}
                  </g>
                )
              }
              const px = (x: number) => (category ? left + band * (x + 0.5) : xMap(x))
              if (plot.chart === 'scatter') {
                return (
                  <g key={s.column} fill={color} fillOpacity={0.75}>
                    {s.points.map(([x, y], j) => (
                      <circle key={j} cx={px(x)} cy={map(y)} r={2.5} />
                    ))}
                  </g>
                )
              }
              const d = s.points.map(([x, y], j) => `${j === 0 ? 'M' : 'L'}${px(x).toFixed(1)},${map(y).toFixed(1)}`).join('')
              return <path key={s.column} d={d} fill="none" stroke={color} strokeWidth={1.6} strokeLinejoin="round" />
            })}
          </g>

          {/* 悬停 */}
          {hover && (
            <line
              x1={category ? left + band * (hover.index + 0.5) : hover.px}
              x2={category ? left + band * (hover.index + 0.5) : hover.px}
              y1={top}
              y2={bottom}
              stroke="currentColor"
              strokeOpacity={0.3}
              strokeDasharray="3 3"
            />
          )}
          <rect x={left} y={top} width={plotW} height={plotH} fill="transparent" onMouseMove={onMove} onMouseLeave={() => setHover(null)} data-export-skip />
        </svg>
      )}
      {hover && hover.rows.length > 0 && (
        <div
          className="pointer-events-none absolute z-10 min-w-36 rounded-control border border-border bg-surface px-2.5 py-2 text-xs shadow-card"
          style={{
            top: margin.top + 8,
            left: (category ? left + band * (hover.index + 0.5) : hover.px) > width / 2 ? undefined : (category ? left + band * (hover.index + 0.5) : hover.px) + 12,
            right: (category ? left + band * (hover.index + 0.5) : hover.px) > width / 2 ? width - (category ? left + band * (hover.index + 0.5) : hover.px) + 12 : undefined,
          }}
        >
          <p className="mb-1 font-medium text-fg">{hover.label}</p>
          {hover.rows.map((row) => (
            <p key={row.name} className="flex items-center gap-2 text-fg-muted">
              <span className="size-2 shrink-0 rounded-full" style={{ background: row.color }} />
              <span className="min-w-0 flex-1 truncate">{row.name}</span>
              <span className="font-mono text-fg tabular-nums">{formatValue(row.value)}</span>
            </p>
          ))}
        </div>
      )}
    </div>
  )
}
