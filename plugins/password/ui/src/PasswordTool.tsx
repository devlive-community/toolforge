import { useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, NumberInput, Panel, Spinner, Switch, Tooltip, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Eye, EyeOff, KeyRound, RefreshCw, ShieldCheck, SlidersHorizontal, TriangleAlert } from 'lucide-react'
import type { Generated, Report } from './types'

const LENGTHS = [12, 16, 24, 32, 64]
const SCORE_TONE = ['bg-danger', 'bg-danger', 'bg-warning', 'bg-success', 'bg-success']
const SCORE_TEXT = ['text-danger', 'text-danger', 'text-warning', 'text-success', 'text-success']

function ToggleRow({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <label className="flex items-center justify-between py-1 text-[13px] text-fg">
      {label}
      {children}
    </label>
  )
}

export function PasswordTool() {
  const { t, errorMessage } = usePlugin()
  const [length, setLength] = useState(20)
  const [count, setCount] = useState(10)
  const [upper, setUpper] = useState(true)
  const [lower, setLower] = useState(true)
  const [digits, setDigits] = useState(true)
  const [symbols, setSymbols] = useState(true)
  const [excludeAmbiguous, setExcludeAmbiguous] = useState(false)
  const [requireEach, setRequireEach] = useState(true)
  const [exclude, setExclude] = useState('')
  const [nonce, setNonce] = useState(0)
  const [check, setCheck] = useState('')
  const [reveal, setReveal] = useState(false)

  const options = { length, count, upper, lower, digits, symbols, excludeAmbiguous, requireEach, exclude }
  const { result, error, pending } = useDebouncedCall<Generated>('generate', options, [
    length, count, upper, lower, digits, symbols, excludeAmbiguous, requireEach, exclude, nonce,
  ])
  const analyzed = useDebouncedCall<Report>(check ? 'analyze' : null, check ? { password: check } : null, [check])
  const report = check && !analyzed.error ? analyzed.result : null

  const classes: [string, boolean, (value: boolean) => void][] = [
    ['upper', upper, setUpper],
    ['lower', lower, setLower],
    ['digits', digits, setDigits],
    ['symbols', symbols, setSymbols],
  ]

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,360px)_minmax(0,1fr)] gap-3">
      <Panel icon={<SlidersHorizontal />} title={t('options.title')} bodyClassName="space-y-4 overflow-auto p-4">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('options.length')}</p>
          <NumberInput value={length} onValueChange={setLength} min={4} max={256} className="w-full" aria-label={t('options.length')} />
          <div className="flex gap-1">
            {LENGTHS.map((value) => (
              <button
                key={value}
                type="button"
                onClick={() => setLength(value)}
                className={cn(
                  'flex-1 rounded-control border px-1.5 py-0.5 text-[11px] tabular-nums outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                  value === length ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:bg-hover',
                )}
              >
                {value}
              </button>
            ))}
          </div>
        </div>

        <div className="rounded-control border border-border px-3 py-1.5">
          {classes.map(([key, checked, set]) => (
            <ToggleRow key={key} label={t(`options.${key}`)}>
              <Switch size="sm" checked={checked} onCheckedChange={set} aria-label={t(`options.${key}`)} />
            </ToggleRow>
          ))}
        </div>
        <div className="rounded-control border border-border px-3 py-1.5">
          <ToggleRow label={t('options.excludeAmbiguous')}>
            <Switch size="sm" checked={excludeAmbiguous} onCheckedChange={setExcludeAmbiguous} aria-label={t('options.excludeAmbiguous')} />
          </ToggleRow>
          <ToggleRow label={t('options.requireEach')}>
            <Switch size="sm" checked={requireEach} onCheckedChange={setRequireEach} aria-label={t('options.requireEach')} />
          </ToggleRow>
        </div>

        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('options.exclude')}</p>
          <Input value={exclude} onChange={(event) => setExclude(event.target.value)} className="font-mono" placeholder={t('options.excludePlaceholder')} aria-label={t('options.exclude')} />
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('options.count')}</p>
          <NumberInput value={count} onValueChange={setCount} min={1} max={500} className="w-full" aria-label={t('options.count')} />
        </div>

        <Button variant="primary" block onClick={() => setNonce((n) => n + 1)}>
          <RefreshCw />
          {t('options.regenerate')}
        </Button>
      </Panel>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel
          icon={<KeyRound />}
          title={t('result.title')}
          extra={
            pending ? (
              <Spinner className="size-3.5 text-fg-subtle" />
            ) : (
              result && !error && <Badge variant="success">{t('result.entropy', { bits: result.entropy, pool: result.poolSize })}</Badge>
            )
          }
          actions={result && !error && <CopyButton text={result.passwords.join('\n')} label={t('result.copyAll')} variant="outline" />}
          bodyClassName="overflow-auto p-2"
        >
          {error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
          ) : !result ? (
            <Empty icon={<KeyRound />} title={t('result.empty')} />
          ) : (
            <ol className="font-mono text-[13px]" data-selectable>
              {result.passwords.map((password, index) => (
                <li key={`${index}-${password}`} className="group flex items-center gap-2 rounded-control px-3 py-1.5 hover:bg-hover">
                  <code className="min-w-0 flex-1 break-all text-fg">{password}</code>
                  <Tooltip content={t('result.check')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('result.check')}
                      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => setCheck(password)}
                    >
                      <ShieldCheck />
                    </Button>
                  </Tooltip>
                  <CopyButton text={password} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                </li>
              ))}
            </ol>
          )}
        </Panel>

        <Panel icon={<ShieldCheck />} title={t('strength.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
          <Input
            type={reveal ? 'text' : 'password'}
            value={check}
            onChange={(event) => setCheck(event.target.value)}
            className="font-mono"
            placeholder={t('strength.placeholder')}
            aria-label={t('strength.label')}
            autoComplete="off"
            spellCheck={false}
            trailing={
              <button
                type="button"
                aria-label={t(reveal ? 'strength.hide' : 'strength.show')}
                onClick={() => setReveal((value) => !value)}
                className="flex rounded-sm text-fg-subtle outline-none hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
              >
                {reveal ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </button>
            }
          />
          {analyzed.error && check ? (
            <p className="text-xs text-danger">{errorMessage(analyzed.error)}</p>
          ) : report ? (
            <div className="space-y-2.5">
              <div className="flex gap-1">
                {[0, 1, 2, 3, 4].map((step) => (
                  <span key={step} className={cn('h-1.5 flex-1 rounded-full', step <= report.score ? SCORE_TONE[report.score] : 'bg-surface-2')} />
                ))}
              </div>
              <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-fg-muted">
                <span className={cn('text-[13px] font-semibold', SCORE_TEXT[report.score])}>{t(`strength.score.${report.score}`)}</span>
                <span>{t('strength.entropy', { bits: report.entropy })}</span>
                <span>{t('strength.length', { count: report.length })}</span>
                <span>
                  {t('strength.crack', {
                    time: report.crackTime.value === 0 ? t(`strength.units.${report.crackTime.unit}`) : t(`strength.units.${report.crackTime.unit}`, { count: report.crackTime.value }),
                  })}
                </span>
              </div>
              {report.warnings.length > 0 && (
                <ul className="space-y-1">
                  {report.warnings.map((warning) => (
                    <li key={warning} className="flex items-center gap-1.5 text-xs text-warning">
                      <TriangleAlert className="size-3.5 shrink-0" />
                      {t(`strength.warnings.${warning}`)}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : (
            <p className="text-xs text-fg-subtle">{t('strength.hint')}</p>
          )}
        </Panel>
      </div>
    </div>
  )
}
