import { useEffect, useMemo, useState } from 'react'
import { Empty, Panel, Spinner } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Monitor, MousePointerClick, Move, ScrollText } from 'lucide-react'
import { RankList, StatCard } from './components'
import { heat, heatColor, legendGradient } from './format'
import type { DisplayMap, Range, Summary } from './types'
import { useFormat } from './useFormat'

const BUTTONS = ['left', 'right', 'middle', 'back', 'forward', 'other']

/** 各显示器按实际排列摆放，点击次数画成热力格 */
function ScreenMap({ displays }: { displays: DisplayMap[] }) {
  const { t } = usePlugin()
  const format = useFormat()
  const [hover, setHover] = useState<{ display: string; count: number } | null>(null)
  const box = useMemo(() => {
    const left = Math.min(...displays.map((d) => d.x))
    const top = Math.min(...displays.map((d) => d.y))
    const right = Math.max(...displays.map((d) => d.x + d.width))
    const bottom = Math.max(...displays.map((d) => d.y + d.height))
    return { left, top, width: right - left, height: bottom - top }
  }, [displays])
  const pad = box.width * 0.01

  return (
    <div className="flex flex-col gap-2">
      <div className="h-4 text-xs tabular-nums text-fg-muted">
        {hover ? `${hover.display} · ${t('count', { count: hover.count, value: format.count(hover.count) })}` : t('mouse.hint')}
      </div>
      <svg
        viewBox={`${box.left - pad} ${box.top - pad} ${box.width + pad * 2} ${box.height + pad * 2}`}
        className="max-h-[420px] w-full"
        onMouseLeave={() => setHover(null)}
      >
        {displays.map((d) => {
          const cw = d.width / d.cols
          const ch = d.height / d.rows
          const radius = Math.min(d.width, d.height) * 0.015
          const safe = d.id.replace(/[^a-z0-9]/gi, '-')
          const clip = `input-stats-clip-${safe}`
          const glow = `input-stats-glow-${safe}`
          return (
            <g key={d.id}>
              <clipPath id={clip}>
                <rect x={d.x} y={d.y} width={d.width} height={d.height} rx={radius} />
              </clipPath>
              {/* 按格子大小模糊，把离散的格子晕染成连续的热区 */}
              <filter id={glow} x="-10%" y="-10%" width="120%" height="120%">
                <feGaussianBlur stdDeviation={cw * 0.9} />
              </filter>
              <rect
                x={d.x}
                y={d.y}
                width={d.width}
                height={d.height}
                rx={radius}
                className="fill-surface-2 stroke-border-strong"
                strokeWidth={box.width / 500}
              />
              <g clipPath={`url(#${clip})`}>
                <g filter={`url(#${glow})`}>
                {d.cells.map(([col, row, count]) => {
                  const level = heat(count, d.max)
                  return (
                    <rect
                      key={`${col}-${row}`}
                      x={d.x + col * cw}
                      y={d.y + row * ch}
                      width={cw}
                      height={ch}
                      style={{ fill: heatColor(level), fillOpacity: 0.45 + level * 0.55 }}
                    />
                  )
                })}
                </g>
              </g>
              {d.cells.map(([col, row, count]) => (
                <rect
                  key={`hit-${col}-${row}`}
                  x={d.x + col * cw}
                  y={d.y + row * ch}
                  width={cw}
                  height={ch}
                  className="fill-transparent"
                  onMouseEnter={() => setHover({ display: `${Math.round(d.width)}×${Math.round(d.height)}`, count })}
                />
              ))}
              <text
                x={d.x + d.width / 2}
                y={d.y + d.height * 0.94}
                textAnchor="middle"
                className="fill-fg-subtle"
                style={{ fontSize: Math.max(d.height * 0.045, box.width * 0.012) }}
              >
                {`${Math.round(d.width)} × ${Math.round(d.height)} · ${t('count', { count: d.total, value: format.count(d.total) })}`}
              </text>
            </g>
          )
        })}
      </svg>
      <div className="flex items-center gap-2 text-xs text-fg-muted">
        <span>{t('legend.less')}</span>
        <span className="h-2 w-32 rounded-full" style={{ background: legendGradient }} />
        <span>{t('legend.more')}</span>
      </div>
    </div>
  )
}

export function Mouse({ summary, range, version }: { summary: Summary; range: Range; version: number }) {
  const { t, call } = usePlugin()
  const format = useFormat()
  const [displays, setDisplays] = useState<{ key: string; maps: DisplayMap[] } | null>(null)
  const key = `${range}:${version}`

  useEffect(() => {
    let alive = true
    call<DisplayMap[]>('click_map', { range })
      .then((maps) => alive && setDisplays({ key, maps }))
      .catch(() => alive && setDisplays({ key, maps: [] }))
    return () => {
      alive = false
    }
  }, [call, range, key])

  const buttons = BUTTONS.map((b) => ({ key: b, label: t(`buttons.${b}`), count: summary.buttons.find((x) => x.name === b)?.count ?? 0 })).filter(
    (b) => b.count > 0 || b.key === 'left' || b.key === 'right',
  )
  const left = summary.buttons.find((b) => b.name === 'left')?.count ?? 0
  const right = summary.buttons.find((b) => b.name === 'right')?.count ?? 0

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
        <StatCard icon={<MousePointerClick />} label={t('stats.clicks')} value={format.count(summary.totals.clicks)} />
        <StatCard
          icon={<MousePointerClick />}
          label={t('mouse.ratio')}
          value={right > 0 ? `${format.decimal(left / right)} : 1` : '—'}
          hint={t('mouse.ratioHint')}
        />
        <StatCard icon={<Move />} label={t('stats.distance')} value={format.distance(summary.totals.distance)} hint={t('stats.distanceHint')} />
        <StatCard icon={<ScrollText />} label={t('stats.scroll')} value={t('unit.lines', { value: format.count(summary.totals.scroll) })} />
      </div>
      <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
        <Panel title={t('mouse.heatmap')} icon={<Monitor />} bodyClassName="p-4">
          {!displays ? (
            <div className="flex h-48 items-center justify-center">
              <Spinner />
            </div>
          ) : displays.maps.length === 0 ? (
            <Empty icon={<Monitor />} title={t('empty.clicks')} />
          ) : (
            <ScreenMap displays={displays.maps} />
          )}
        </Panel>
        <Panel title={t('mouse.buttons')} icon={<MousePointerClick />}>
          <RankList items={buttons} format={format.count} empty={t('empty.clicks')} />
        </Panel>
      </div>
    </div>
  )
}
