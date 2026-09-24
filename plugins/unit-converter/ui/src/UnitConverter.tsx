import { useState, type ReactNode } from 'react'
import { Input, Panel, Select, Spinner, Tooltip, cn } from '@toolforge/ui'
import { useCopy, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import {
  AudioWaveform,
  Check,
  Clock,
  Compass,
  Copy,
  Database,
  Gauge,
  Grid2x2,
  Milk,
  Ruler,
  Thermometer,
  Weight,
  Wifi,
  Zap,
  Flame,
  ArrowDownToLine,
} from 'lucide-react'
import type { CategoryInfo, Converted } from './types'

const ICONS: Record<string, ReactNode> = {
  length: <Ruler />,
  mass: <Weight />,
  temperature: <Thermometer />,
  area: <Grid2x2 />,
  volume: <Milk />,
  speed: <Gauge />,
  time: <Clock />,
  data: <Database />,
  datarate: <Wifi />,
  pressure: <ArrowDownToLine />,
  energy: <Flame />,
  power: <Zap />,
  angle: <Compass />,
  frequency: <AudioWaveform />,
}

/** 各分类默认的输入单位 */
const DEFAULT_UNIT: Record<string, string> = {
  length: 'km',
  mass: 'kg',
  temperature: 'c',
  area: 'm2',
  volume: 'l',
  speed: 'kmh',
  time: 'h',
  data: 'gb',
  datarate: 'mbps',
  pressure: 'atm',
  energy: 'kwh',
  power: 'kw',
  angle: 'deg',
  frequency: 'mhz',
}

const PRECISIONS = [4, 6, 8, 10, 12, 15]

export function UnitConverter() {
  const { t, errorMessage } = usePlugin()
  const { copy } = useCopy()
  const [category, setCategory] = useState('length')
  const [units, setUnits] = useState<Record<string, string>>(DEFAULT_UNIT)
  const [value, setValue] = useState('1')
  const [precision, setPrecision] = useState(10)
  const [copied, setCopied] = useState<string | null>(null)

  const catalog = useDebouncedCall<CategoryInfo[]>('catalog', {}, [])
  const categories = catalog.result ?? []
  const current = categories.find((c) => c.id === category)
  const from = units[category] ?? current?.base ?? ''

  const args = current && value.trim() ? { category, from, value, precision } : null
  const { result, error, pending } = useDebouncedCall<Converted>(args ? 'convert' : null, args, [category, from, value, precision])
  const converted = error ? null : result

  const unitName = (id: string) => t(`units.${id}`)
  const setFrom = (unit: string) => setUnits((prev) => ({ ...prev, [category]: unit }))

  const copyValue = (unit: string, text: string) => {
    copy(text)
    setCopied(unit)
    setTimeout(() => setCopied((c) => (c === unit ? null : c)), 1200)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[200px_minmax(0,1fr)] gap-3">
      <nav className="flex min-h-0 flex-col gap-0.5 overflow-auto rounded-card border border-border bg-surface p-2 shadow-card">
        {categories.map((c) => (
          <button
            key={c.id}
            type="button"
            aria-current={c.id === category}
            onClick={() => setCategory(c.id)}
            className={cn(
              'flex items-center gap-2.5 rounded-control px-2.5 py-2 text-left text-[13px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-4',
              c.id === category ? 'bg-primary-soft font-medium text-primary-fg' : 'text-fg hover:bg-hover',
            )}
          >
            <span className={c.id === category ? 'text-primary-fg' : 'text-fg-muted'}>{ICONS[c.id]}</span>
            {t(`categories.${c.id}`)}
          </button>
        ))}
      </nav>

      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
        <section className="space-y-2 rounded-card border border-border bg-surface p-4 shadow-card">
          <div className="flex items-center gap-3">
            <Input
              value={value}
              onChange={(event) => setValue(event.target.value)}
              size="lg"
              className="font-mono"
              wrapperClassName="flex-1"
              invalid={!!error}
              trailing={pending && <Spinner className="size-3.5" />}
              placeholder={t('input.placeholder')}
              aria-label={t('input.value')}
            />
            <Select<string>
              value={from || null}
              onValueChange={setFrom}
              className="w-56"
              aria-label={t('input.unit')}
              options={(current?.units ?? []).map((u) => ({ value: u.id, label: unitName(u.id) === u.symbol ? u.symbol : `${unitName(u.id)} (${u.symbol})` }))}
            />
            <Select<number>
              value={precision}
              onValueChange={setPrecision}
              className="w-36"
              aria-label={t('input.precision')}
              options={PRECISIONS.map((p) => ({ value: p, label: t('input.digits', { count: p }) }))}
            />
          </div>
          {error ? <p className="text-xs text-danger">{errorMessage(error)}</p> : <p className="text-xs text-fg-subtle">{t('input.hint')}</p>}
        </section>

        <Panel
          icon={current && ICONS[current.id]}
          title={current ? t(`categories.${current.id}`) : ''}
          bodyClassName="overflow-auto p-3"
        >
          <div className="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-2">
            {converted?.results.map((r) => {
              const source = r.unit === from
              return (
                <div
                  key={r.unit}
                  className={cn(
                    'group flex items-center gap-2 rounded-card border px-3 py-2.5 transition-colors',
                    source ? 'border-primary bg-primary-soft/40' : 'border-border hover:bg-hover',
                  )}
                >
                  <button
                    type="button"
                    onClick={() => {
                      setFrom(r.unit)
                      setValue(r.value)
                    }}
                    className="min-w-0 flex-1 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    title={t('results.useAsInput')}
                  >
                    <p className="truncate font-mono text-[15px] font-semibold text-fg tabular-nums" data-selectable>
                      {r.value}
                    </p>
                    <p className="truncate text-xs text-fg-muted">{unitName(r.unit) === r.symbol ? r.symbol : `${unitName(r.unit)} · ${r.symbol}`}</p>
                  </button>
                  <Tooltip content={t('common:copy')}>
                    <button
                      type="button"
                      aria-label={t('common:copy')}
                      onClick={() => copyValue(r.unit, r.value)}
                      className="flex size-7 shrink-0 items-center justify-center rounded-control text-fg-subtle opacity-0 outline-none transition-opacity group-hover:opacity-100 hover:bg-hover hover:text-fg focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-3.5"
                    >
                      {copied === r.unit ? <Check className="text-success" /> : <Copy />}
                    </button>
                  </Tooltip>
                </div>
              )
            })}
          </div>
        </Panel>
      </div>
    </div>
  )
}
