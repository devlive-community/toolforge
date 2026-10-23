import { useState } from 'react'
import { Badge, Button, Input, Panel, Select, Spinner, Tooltip } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChartColumn, Filter as FilterIcon, Plus, X } from 'lucide-react'
import { OPS, needsValue, type Filter, type Op, type Spec, type Stats, type TableInfo } from './types'

interface ColumnPanelProps {
  info: TableInfo
  spec: Spec
  column: number
  name: string
  onAddFilter: (filter: Filter) => void
  onClose: () => void
}

const format = (value: number) => (Number.isInteger(value) ? value.toLocaleString() : value.toLocaleString(undefined, { maximumFractionDigits: 4 }))

export function ColumnPanel({ info, spec, column, name, onAddFilter, onClose }: ColumnPanelProps) {
  const { t } = usePlugin()
  const { result: stats, pending } = useDebouncedCall<Stats>('stats', { id: info.id, column, ...spec }, [info.id, column, JSON.stringify(spec)])
  const kind = info.columns[column].kind
  const numeric = kind === 'integer' || kind === 'float'
  const [op, setOp] = useState<Op>(numeric ? 'greater' : 'contains')
  const [value, setValue] = useState('')

  const add = () => {
    if (needsValue(op) && !value.trim()) return
    onAddFilter({ column, op, value: needsValue(op) ? value : '' })
    setValue('')
  }

  const rows: [string, string][] = stats
    ? [
        [t('stats.count'), format(stats.count)],
        [t('stats.empty'), format(stats.empty)],
        [t('stats.distinct'), stats.distinctCapped ? t('stats.capped', { count: stats.distinct }) : format(stats.distinct)],
        ...(stats.min !== null && stats.max !== null
          ? ([
              [t('stats.min'), format(stats.min)],
              [t('stats.max'), format(stats.max)],
              [t('stats.mean'), format(stats.mean ?? 0)],
              [t('stats.sum'), format(stats.sum ?? 0)],
            ] as [string, string][])
          : []),
        ...(stats.minLength !== null
          ? ([[t('stats.length'), t('stats.lengthRange', { min: stats.minLength, max: stats.maxLength })]] as [string, string][])
          : []),
      ]
    : []
  const topMax = stats?.top[0]?.count ?? 1

  return (
    <Panel
      icon={<ChartColumn />}
      title={<span className="truncate">{name}</span>}
      extra={<Badge>{t(`kinds.${kind}`)}</Badge>}
      actions={
        <Button size="icon-sm" variant="ghost" aria-label={t('stats.close')} onClick={onClose}>
          <X />
        </Button>
      }
      bodyClassName="overflow-auto p-3 space-y-4"
    >
      <div className="space-y-1.5">
        <p className="text-xs font-semibold text-fg-muted">{t('filters.add')}</p>
        <Select<Op> size="sm" value={op} onValueChange={setOp} aria-label={t('filters.add')} options={OPS.map((o) => ({ value: o, label: t(`filters.ops.${o}`) }))} />
        {needsValue(op) && (
          <Input
            size="sm"
            value={value}
            onChange={(event) => setValue(event.target.value)}
            onKeyDown={(event) => event.key === 'Enter' && add()}
            placeholder={t('filters.value')}
            aria-label={t('filters.value')}
          />
        )}
        <Button size="sm" block onClick={add} disabled={needsValue(op) && !value.trim()}>
          <Plus />
          {t('filters.add')}
        </Button>
      </div>

      {!stats ? (
        <div className="flex justify-center py-6">
          <Spinner />
        </div>
      ) : (
        <>
          <dl className={pending ? 'opacity-60' : undefined}>
            {rows.map(([label, text]) => (
              <div key={label} className="flex items-baseline justify-between gap-3 py-1 text-[12.5px]">
                <dt className="text-fg-muted">{label}</dt>
                <dd className="truncate font-mono text-fg tabular-nums" data-selectable>
                  {text}
                </dd>
              </div>
            ))}
          </dl>
          {stats.top.length > 0 && (
            <div className="space-y-1">
              <p className="text-xs font-semibold text-fg-muted">{t('stats.top')}</p>
              {stats.top.map((item) => (
                <div key={item.value} className="group relative flex items-center gap-2 overflow-hidden rounded-control px-2 py-1 text-[12.5px]">
                  <span className="absolute inset-y-0 left-0 bg-primary-soft" style={{ width: `${(item.count / topMax) * 100}%` }} />
                  <span className="relative min-w-0 flex-1 truncate text-fg" title={item.value} data-selectable>
                    {item.value}
                  </span>
                  <span className="relative text-fg-muted tabular-nums">{item.count.toLocaleString()}</span>
                  <CopyButton text={item.value} className="relative -my-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                  <Tooltip content={t('stats.only')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('stats.only')}
                      className="relative -my-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => onAddFilter({ column, op: 'equals', value: item.value })}
                    >
                      <FilterIcon />
                    </Button>
                  </Tooltip>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </Panel>
  )
}
