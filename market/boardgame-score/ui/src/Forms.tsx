import { useState } from 'react'
import { Button, Input, SegmentedControl, Tooltip } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Minus, Plus, Save } from 'lucide-react'
import type { Editor, Game, Op } from './types'

export function GameForm({
  editor,
  game,
  busy,
  save,
  cancel,
}: {
  editor: Editor
  game: Game | null
  busy: boolean
  save: (op: Op) => void
  cancel: () => void
}) {
  const { t } = usePlugin()
  const [name, setName] = useState(editor.kind === 'copy' ? t('form.copyName', { name: game?.name }) : editor.kind === 'rename' ? (game?.name ?? '') : '')
  const [players, setPlayers] = useState(
    editor.kind === 'rename' ? (game?.players.map((p) => p.name) ?? []) : [t('form.defaultPlayer', { n: 1 }), t('form.defaultPlayer', { n: 2 })],
  )
  const [direction, setDirection] = useState<'high' | 'low'>('high')
  const [roundLimit, setRoundLimit] = useState('')
  const [steps, setSteps] = useState(['1', '5'])
  const [values, setValues] = useState(editor.round?.scores.map(String) ?? [])
  const submit = () => {
    if (editor.kind === 'new') save({ type: 'new', name, players, direction, roundLimit, steps })
    else if (editor.kind === 'copy') save({ type: 'copy', name })
    else if (editor.kind === 'rename') save({ type: 'rename', name, players })
    else save({ type: 'editRound', round: editor.round?.number, values })
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <div>
        <h2 className="text-lg font-semibold text-fg">{t(`form.${editor.kind}`)}</h2>
        <p className="mt-1 text-xs leading-relaxed text-fg-muted">{t(`form.${editor.kind}Hint`)}</p>
      </div>
      <div className="min-h-0 flex-1 space-y-4 overflow-auto pr-1">
        {editor.kind !== 'edit' && (
          <label className="block space-y-1.5">
            <span className="text-xs font-medium text-fg">{t('form.name')}</span>
            <Input aria-label={t('form.name')} value={name} onChange={(e) => setName(e.target.value)} disabled={busy} maxLength={80} autoFocus />
          </label>
        )}
        {(editor.kind === 'new' || editor.kind === 'rename') && (
          <div className="space-y-2">
            <p className="text-xs font-medium text-fg">{t('form.players')}</p>
            {players.map((player, i) => (
              <div key={i} className="flex items-center gap-2">
                <span className="w-7 shrink-0 text-xs font-medium text-fg-subtle">{t('seat', { n: i + 1 })}</span>
                <Input
                  wrapperClassName="min-w-0 flex-1"
                  aria-label={t('form.player', { n: i + 1 })}
                  value={player}
                  onChange={(e) => setPlayers(players.map((p, index) => (index === i ? e.target.value : p)))}
                  disabled={busy}
                  maxLength={80}
                />
                {editor.kind === 'new' && (
                  <Tooltip content={t('form.remove')}>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      disabled={busy || players.length <= 2}
                      aria-label={t('form.remove')}
                      onClick={() => setPlayers(players.filter((_, index) => index !== i))}
                    >
                      <Minus />
                    </Button>
                  </Tooltip>
                )}
              </div>
            ))}
            {editor.kind === 'new' && (
              <Button
                size="sm"
                variant="outline"
                disabled={busy || players.length >= 12}
                onClick={() => setPlayers([...players, t('form.defaultPlayer', { n: players.length + 1 })])}
              >
                <Plus />
                {t('form.add')}
              </Button>
            )}
          </div>
        )}
        {editor.kind === 'new' && (
          <>
            <div className={busy ? 'pointer-events-none opacity-50' : ''}>
              <SegmentedControl
                value={direction}
                onValueChange={setDirection}
                aria-label={t('form.direction')}
                options={[
                  { value: 'high', label: t('direction.high') },
                  { value: 'low', label: t('direction.low') },
                ]}
              />
            </div>
            <label className="block space-y-1.5">
              <span className="text-xs font-medium text-fg">{t('form.limit')}</span>
              <Input
                aria-label={t('form.limit')}
                placeholder={t('form.manual')}
                value={roundLimit}
                onChange={(e) => setRoundLimit(e.target.value)}
                inputMode="numeric"
                disabled={busy}
                maxLength={4}
              />
            </label>
            <div className="grid grid-cols-2 gap-3">
              {steps.map((step, i) => (
                <label key={i} className="space-y-1.5">
                  <span className="text-xs font-medium text-fg">{t('form.step', { n: i + 1 })}</span>
                  <Input
                    aria-label={t('form.step', { n: i + 1 })}
                    value={step}
                    onChange={(e) => setSteps(steps.map((s, index) => (index === i ? e.target.value : s)))}
                    inputMode="numeric"
                    disabled={busy}
                    maxLength={8}
                  />
                </label>
              ))}
            </div>
          </>
        )}
        {editor.kind === 'edit' && game && (
          <div className="space-y-3">
            {game.players.map((p, i) => (
              <label key={p.seat} className="flex items-center gap-3">
                <span className="min-w-0 flex-1 truncate text-sm text-fg">
                  {t('seat', { n: p.seat })} {p.name}
                </span>
                <span className="text-xs text-fg-subtle">{String(editor.round?.scores[i])}</span>
                <span className="text-fg-subtle">{t('common.arrow')}</span>
                <Input
                  wrapperClassName="w-32"
                  aria-label={t('entry.score', { n: p.seat })}
                  value={values[i]}
                  onChange={(e) => setValues(values.map((v, index) => (index === i ? e.target.value : v)))}
                  disabled={busy}
                  inputMode="text"
                  maxLength={12}
                />
              </label>
            ))}
          </div>
        )}
      </div>
      <div className="flex shrink-0 gap-2 border-t border-border pt-3">
        <Button variant="primary" disabled={busy} onClick={submit}>
          {t('common.save')}
        </Button>
        <Button variant="ghost" disabled={busy} onClick={cancel}>
          {t('common.cancel')}
        </Button>
      </div>
    </div>
  )
}

export function RoundEntry({ game, busy, save, onDirty }: { game: Game; busy: boolean; save: (op: Op) => void; onDirty: (dirty: boolean) => void }) {
  const { t } = usePlugin()
  const initial = game.draft.map((v) => (v === null ? '' : String(v)))
  const [values, setValues] = useState(initial)
  const [dirty, setDirty] = useState(false)
  const change = (i: number, text: string) => {
    setValues(values.map((v, index) => (index === i ? text : v)))
    setDirty(true)
    onDirty(true)
  }
  const payload = { values, expectedRound: game.nextRound }
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-semibold text-fg">{t('entry.round', { n: game.nextRound })}</h3>
        <span role="status" className={dirty ? 'text-xs text-warning' : 'text-xs text-fg-subtle'}>
          {t(dirty ? 'entry.unsaved' : 'entry.saved')}
        </span>
      </div>
      <p className="text-xs leading-relaxed text-fg-muted">{t('entry.hint')}</p>
      <div className="min-h-0 flex-1 space-y-2 overflow-auto pr-1">
        {game.players.map((p, i) => (
          <div key={p.seat} className="rounded-card border border-border bg-surface-2 p-3">
            <div className="mb-2 flex items-center justify-between gap-3">
              <span className="min-w-0 truncate text-sm font-medium text-fg">
                <span className="mr-2 text-fg-subtle">{t('seat', { n: p.seat })}</span>
                {p.name}
              </span>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Input
                wrapperClassName="min-w-24 flex-1"
                value={values[i]}
                onChange={(e) => change(i, e.target.value)}
                aria-label={t('entry.score', { n: p.seat })}
                placeholder={t('entry.empty')}
                inputMode="text"
                maxLength={12}
                disabled={busy}
              />
              <div className="flex gap-1">
                {game.steps.map((step) => (
                  <span key={step} className="inline-flex overflow-hidden rounded-control border border-border">
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      aria-label={t('entry.adjust', { n: p.seat, delta: `-${step}` })}
                      onClick={() => save({ type: 'adjust', ...payload, seat: p.seat, delta: `-${step}` })}
                    >{`−${step}`}</Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      aria-label={t('entry.adjust', { n: p.seat, delta: `+${step}` })}
                      onClick={() => save({ type: 'adjust', ...payload, seat: p.seat, delta: String(step) })}
                    >{`+${step}`}</Button>
                  </span>
                ))}
              </div>
            </div>
          </div>
        ))}
      </div>
      <div className="flex shrink-0 flex-wrap gap-2 border-t border-border pt-3">
        <Button variant="primary" disabled={busy} onClick={() => save({ type: 'confirmRound', ...payload })}>
          {t('entry.confirm')}
        </Button>
        <Button variant="outline" disabled={busy || !dirty} onClick={() => save({ type: 'draft', ...payload })}>
          <Save />
          {t('entry.save')}
        </Button>
        {dirty && (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => {
              setValues(initial)
              setDirty(false)
              onDirty(false)
            }}
          >
            {t('entry.discard')}
          </Button>
        )}
        {!dirty && game.draft.some((v) => v !== null) && (
          <Button variant="ghost" disabled={busy} onClick={() => save({ type: 'draft', values: game.players.map(() => ''), expectedRound: game.nextRound })}>
            {t('entry.clear')}
          </Button>
        )}
      </div>
    </div>
  )
}
