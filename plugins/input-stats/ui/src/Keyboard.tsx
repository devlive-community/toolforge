import { useMemo, useState, type CSSProperties } from 'react'
import { Panel, SegmentedControl, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Command, Keyboard as KeyboardIcon, ListOrdered } from 'lucide-react'
import { RankList } from './components'
import { heat, heatStyle, heatText, legendGradient } from './format'
import { FULL_ONLY, comboLabel, keyLabel, keyboardLayout, type LayoutKind } from './layouts'
import type { Platform, Summary } from './types'
import { useFormat } from './useFormat'

const SIDED = /^(Shift|Control|Alt|Meta)(Left|Right)$/

/** 列表中使用的键名：特殊键翻译，左右修饰键注明左右 */
export function useKeyName(platform: Platform, os: string) {
  const { t } = usePlugin()
  return (code: string) => {
    const custom = t(`keys.${code}`, { defaultValue: '' })
    if (custom) return custom
    const label = keyLabel(code, platform, os)
    const side = SIDED.exec(code)?.[2]
    return side ? t(`side.${side.toLowerCase()}`, { key: label }) : label
  }
}

export function Keyboard({ summary, platform, os }: { summary: Summary; platform: Platform; os: string }) {
  const { t } = usePlugin()
  const format = useFormat()
  const keyName = useKeyName(platform, os)
  const counts = useMemo(() => new Map(summary.keys.map((k) => [k.name, k.count])), [summary.keys])
  const total = summary.totals.keys
  const autoKind: LayoutKind = summary.keys.some((k) => FULL_ONLY.test(k.name)) ? 'full' : 'compact'
  const [chosen, setChosen] = useState<LayoutKind | null>(null)
  const kind = chosen ?? autoKind
  const layout = useMemo(() => keyboardLayout(kind, platform), [kind, platform])
  const shown = useMemo(() => new Set(layout.keys.map((k) => k.code)), [layout])
  const max = summary.keys[0]?.count ?? 0
  const others = summary.keys.filter((k) => !shown.has(k.name))
  const [hover, setHover] = useState<string | null>(null)

  const describe = (code: string) => {
    const count = counts.get(code) ?? 0
    return `${keyName(code)} · ${t('count', { count, value: format.count(count) })} · ${format.percent(total > 0 ? count / total : 0)}`
  }

  // 1u 的宽度：键帽文字随键盘宽度缩放
  const unit = { '--u': `calc(100cqw / ${layout.width})` } as CSSProperties

  return (
    <div className="flex flex-col gap-4">
      <Panel
        title={t('keyboard.title')}
        icon={<KeyboardIcon />}
        extra={<span className="truncate text-xs tabular-nums text-fg-muted">{hover ? describe(hover) : t('keyboard.hint')}</span>}
        actions={
          <SegmentedControl
            size="sm"
            value={kind}
            onValueChange={setChosen}
            options={[
              { value: 'compact', label: t('keyboard.compact') },
              { value: 'full', label: t('keyboard.full') },
            ]}
            aria-label={t('keyboard.layout')}
          />
        }
        bodyClassName="p-4"
      >
        <div className="@container mx-auto w-full max-w-[1100px]">
          <div
            className="relative w-full"
            style={{ ...unit, aspectRatio: `${layout.width} / ${layout.height}` }}
            onMouseLeave={() => setHover(null)}
          >
            {layout.keys.map((key) => {
              const count = counts.get(key.code) ?? 0
              const level = heat(count, max)
              const label = keyLabel(key.code, platform, os)
              return (
                <div
                  key={`${key.code}-${key.x}-${key.y}`}
                  className="absolute p-[calc(var(--u)*0.05)]"
                  style={{
                    left: `${(key.x / layout.width) * 100}%`,
                    top: `${(key.y / layout.height) * 100}%`,
                    width: `${(key.w / layout.width) * 100}%`,
                    height: `${(key.h / layout.height) * 100}%`,
                  }}
                  onMouseEnter={() => setHover(key.code)}
                >
                  <div
                    aria-label={describe(key.code)}
                    className={cn(
                      'flex h-full w-full flex-col items-center justify-center overflow-hidden rounded-[calc(var(--u)*0.12)] border border-border bg-surface-2 shadow-card transition-colors',
                      hover === key.code && 'ring-2 ring-ring',
                      heatText(level),
                    )}
                    style={heatStyle(level)}
                  >
                    <span className="truncate px-0.5 text-[length:calc(var(--u)*0.3)] leading-none font-medium">{label}</span>
                    {count > 0 && key.h >= 1 && (
                      <span className="mt-[calc(var(--u)*0.06)] truncate text-[length:calc(var(--u)*0.2)] leading-none tabular-nums opacity-75">
                        {format.count(count)}
                      </span>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </div>
        <div className="mx-auto mt-4 flex max-w-[1100px] flex-wrap items-center gap-x-6 gap-y-2 text-xs text-fg-muted">
          <div className="flex items-center gap-2">
            <span>{t('legend.less')}</span>
            <span className="h-2 w-32 rounded-full" style={{ background: legendGradient }} />
            <span>{t('legend.more')}</span>
          </div>
          <span className="tabular-nums">{t('keyboard.total', { value: format.count(total) })}</span>
          <span className="tabular-nums">{t('keyboard.distinct', { count: summary.keys.length })}</span>
        </div>
        {others.length > 0 && (
          <div className="mx-auto mt-3 flex max-w-[1100px] flex-wrap items-center gap-1.5">
            <span className="mr-1 text-xs text-fg-subtle">{t('keyboard.others')}</span>
            {others.map((k) => (
              <span key={k.name} className="rounded-[5px] border border-border bg-surface-2 px-1.5 py-0.5 text-xs text-fg-muted">
                {keyName(k.name)} <span className="tabular-nums text-fg-subtle">{format.count(k.count)}</span>
              </span>
            ))}
          </div>
        )}
      </Panel>
      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title={t('keyboard.top')} icon={<ListOrdered />}>
          <RankList
            items={summary.keys.slice(0, 15).map((k) => ({ key: k.name, label: keyName(k.name), count: k.count }))}
            format={format.count}
            empty={t('empty.keys')}
          />
        </Panel>
        <Panel title={t('keyboard.combos')} icon={<Command />}>
          <RankList
            items={summary.combos.slice(0, 15).map((c) => ({
              key: c.name,
              label: <span className="font-mono">{comboLabel(c.name, platform, os)}</span>,
              count: c.count,
            }))}
            format={format.count}
            empty={t('empty.combos')}
          />
        </Panel>
      </div>
    </div>
  )
}
