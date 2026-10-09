import { Button, Empty, NumberInput, Panel, Switch, cn } from '@toolforge/ui'
import { CopyButton, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Hash, Shuffle, SlidersHorizontal } from 'lucide-react'
import { Field, MetaBadge, SeedField, ToggleRow, useRun, type Meta, type SeedProps } from './shared'

interface Numbers extends Meta {
  values: number[]
  sum: string
}

const PRESETS: [number, number][] = [
  [1, 6],
  [1, 10],
  [1, 100],
  [1, 1000],
]

export function NumbersTab({ seed, setSeed }: SeedProps) {
  const { t, errorMessage } = usePlugin()
  const [min, setMin] = usePluginState('numbers.min', 1)
  const [max, setMax] = usePluginState('numbers.max', 100)
  const [count, setCount] = usePluginState('numbers.count', 1)
  const [unique, setUnique] = usePluginState('numbers.unique', true)
  const [sorted, setSorted] = usePluginState('numbers.sorted', false)
  const { result, error, pending, round, run } = useRun<Numbers>()
  const generate = () => run('numbers', { min, max, count, unique, sorted, seed })
  const big = result?.values.length === 1

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,340px)_minmax(0,1fr)] gap-3">
      <Panel icon={<SlidersHorizontal />} title={t('options.title')} bodyClassName="space-y-4 overflow-auto p-4">
        <Field label={t('numbers.range')}>
          <div className="flex items-center gap-2">
            <NumberInput value={min} onValueChange={setMin} className="min-w-0 flex-1" aria-label={t('numbers.min')} />
            <span className="text-fg-subtle">{'–'}</span>
            <NumberInput value={max} onValueChange={setMax} className="min-w-0 flex-1" aria-label={t('numbers.max')} />
          </div>
          <div className="flex gap-1">
            {PRESETS.map(([a, b]) => (
              <button
                key={`${a}-${b}`}
                type="button"
                onClick={() => {
                  setMin(a)
                  setMax(b)
                }}
                className={cn(
                  'flex-1 whitespace-nowrap rounded-control border px-1.5 py-0.5 text-[11px] tabular-nums outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                  a === min && b === max ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:bg-hover',
                )}
              >
                {`${a}–${b}`}
              </button>
            ))}
          </div>
        </Field>
        <Field label={t('numbers.count')}>
          <NumberInput value={count} onValueChange={setCount} min={1} max={10000} className="w-full" aria-label={t('numbers.count')} />
        </Field>
        <div className="rounded-control border border-border px-3 py-1.5">
          <ToggleRow label={t('numbers.unique')}>
            <Switch size="sm" checked={unique} onCheckedChange={setUnique} aria-label={t('numbers.unique')} />
          </ToggleRow>
          <ToggleRow label={t('numbers.sorted')}>
            <Switch size="sm" checked={sorted} onCheckedChange={setSorted} aria-label={t('numbers.sorted')} />
          </ToggleRow>
        </div>
        <SeedField seed={seed} setSeed={setSeed} />
        <Button variant="primary" block onClick={generate} disabled={pending}>
          <Shuffle />
          {t('numbers.generate')}
        </Button>
      </Panel>

      <Panel
        icon={<Hash />}
        title={t('result.title')}
        extra={!error && <MetaBadge meta={result} seed={seed} />}
        actions={result && !error && <CopyButton text={result.values.join('\n')} label={t('result.copy')} variant="outline" />}
        footer={
          result &&
          !error &&
          result.values.length > 1 && (
            <span>
              {t('numbers.summary', {
                count: result.values.length,
                sum: result.sum,
              })}
            </span>
          )
        }
        bodyClassName="overflow-auto p-4"
      >
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <Empty icon={<Hash />} title={t('result.empty')} description={t('numbers.emptyHint')} />
        ) : big ? (
          <div key={round} className="flex h-full items-center justify-center">
            <span className="font-mono text-7xl font-semibold text-fg tabular-nums" data-selectable>
              {result.values[0]}
            </span>
          </div>
        ) : (
          <div key={round} className="flex flex-wrap gap-1.5" data-selectable>
            {result.values.map((v, i) => (
              <span key={i} className="rounded-control border border-border bg-surface-2 px-2.5 py-1 font-mono text-[13px] text-fg tabular-nums">
                {v}
              </span>
            ))}
          </div>
        )}
      </Panel>
    </div>
  )
}
