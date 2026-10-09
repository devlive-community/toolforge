import { useState } from 'react'
import { Button, Empty, NumberInput, Panel, SegmentedControl, cn } from '@toolforge/ui'
import { CopyButton, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Coins, Dice1, Dice2, Dice3, Dice4, Dice5, Dice6, Dices, SlidersHorizontal } from 'lucide-react'
import { Field, MetaBadge, SeedField, useRun, type Meta, type SeedProps } from './shared'

type Mode = 'dice' | 'coin'

interface Rolls extends Meta {
  rolls: number[]
  sum: number
}

const FACES = [Dice1, Dice2, Dice3, Dice4, Dice5, Dice6]
const SIDES = [4, 6, 8, 10, 12, 20, 100]

export function DiceTab({ seed, setSeed }: SeedProps) {
  const { t, errorMessage } = usePlugin()
  const [mode, setMode] = usePluginState<Mode>('dice.mode', 'dice')
  const [count, setCount] = usePluginState('dice.count', 2)
  const [sides, setSides] = usePluginState('dice.sides', 6)
  const { result, error, pending, round, run } = useRun<Rolls>()
  // 记住发起时的模式与面数，之后切换选项不影响已有结果的展示
  const [rolled, setRolled] = useState<{ mode: Mode; sides: number }>({
    mode,
    sides,
  })
  const roll = () => {
    setRolled({ mode, sides })
    run('dice', { count, sides: mode === 'coin' ? 2 : sides, seed })
  }
  const shownMode = rolled.mode
  const coinLabel = (r: number) => t(r === 1 ? 'dice.heads' : 'dice.tails')
  const heads = result?.rolls.filter((r) => r === 1).length ?? 0
  const copyText = result ? (shownMode === 'coin' ? result.rolls.map(coinLabel) : result.rolls).join(' ') : ''

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,340px)_minmax(0,1fr)] gap-3">
      <Panel icon={<SlidersHorizontal />} title={t('options.title')} bodyClassName="space-y-4 overflow-auto p-4">
        <SegmentedControl<Mode>
          value={mode}
          onValueChange={setMode}
          className="w-full"
          aria-label={t('tabs.dice')}
          options={[
            { value: 'dice', label: t('dice.dice') },
            { value: 'coin', label: t('dice.coin') },
          ]}
        />
        <Field label={t(mode === 'coin' ? 'dice.coins' : 'dice.count')}>
          <NumberInput value={count} onValueChange={setCount} min={1} max={10000} className="w-full" aria-label={t('dice.count')} />
        </Field>
        {mode === 'dice' && (
          <Field label={t('dice.sides')}>
            <NumberInput value={sides} onValueChange={setSides} min={2} max={1000000} className="w-full" aria-label={t('dice.sides')} />
            <div className="flex gap-1">
              {SIDES.map((value) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => setSides(value)}
                  className={cn(
                    'flex-1 rounded-control border px-1 py-0.5 text-[11px] tabular-nums outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                    value === sides ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:bg-hover',
                  )}
                >
                  {`d${value}`}
                </button>
              ))}
            </div>
          </Field>
        )}
        <SeedField seed={seed} setSeed={setSeed} />
        <Button variant="primary" block onClick={roll} disabled={pending}>
          {mode === 'coin' ? <Coins /> : <Dices />}
          {t(mode === 'coin' ? 'dice.flip' : 'dice.roll')}
        </Button>
      </Panel>

      <Panel
        icon={<Dices />}
        title={t('result.title')}
        extra={!error && <MetaBadge meta={result} seed={seed} />}
        actions={result && !error && <CopyButton text={copyText} label={t('result.copy')} variant="outline" />}
        footer={
          result &&
          !error && (
            <span>
              {shownMode === 'coin'
                ? t('dice.coinSummary', {
                    heads,
                    tails: result.rolls.length - heads,
                  })
                : t('dice.summary', {
                    count: result.rolls.length,
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
          <Empty icon={<Dices />} title={t('result.empty')} description={t('dice.emptyHint')} />
        ) : (
          <div key={round} className="flex flex-wrap content-start gap-2" data-selectable>
            {result.rolls.map((r, i) => {
              if (shownMode === 'coin') {
                return (
                  <span
                    key={i}
                    className={cn(
                      'flex size-16 items-center justify-center rounded-full border-2 text-sm font-semibold',
                      r === 1 ? 'border-warning bg-warning-soft text-warning' : 'border-border-strong bg-surface-2 text-fg-muted',
                    )}
                  >
                    {coinLabel(r)}
                  </span>
                )
              }
              const Face = rolled.sides === 6 && r <= 6 ? FACES[r - 1] : null
              return Face ? (
                <Face key={i} aria-label={String(r)} className="size-16 text-fg" strokeWidth={1.25} />
              ) : (
                <span
                  key={i}
                  className="flex h-16 min-w-16 items-center justify-center rounded-card border-2 border-border-strong px-3 font-mono text-2xl font-semibold text-fg tabular-nums"
                >
                  {r}
                </span>
              )
            })}
          </div>
        )}
      </Panel>
    </div>
  )
}
