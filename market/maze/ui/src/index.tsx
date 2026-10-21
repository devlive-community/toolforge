import { useState } from 'react'
import { usePlugin, host } from '@toolforge/plugin-ui-sdk'
import { Badge, Button, Empty, LogViewer, Modal, Panel, SegmentedControl, Spinner, Tooltip, toast } from '@toolforge/ui'
import { Copy, Eye, EyeOff, Flag, Lightbulb, Map, RotateCcw, Square, Undo2 } from 'lucide-react'
import { Board } from './Board'
import { Generator } from './Generator'
import { Paper } from './Paper'
import { useMaze } from './useMaze'
import type { Op } from './types'

export default function MazeMaker() {
  const { t, errorMessage } = usePlugin()
  const engine = useMaze()
  const { view } = engine
  const [tab, setTab] = useState<'play' | 'paper'>('play')
  const [paperBusy, setPaperBusy] = useState(false)
  const [paperEpoch, setPaperEpoch] = useState(0)
  const [confirmation, setConfirmation] = useState<Op | null>(null)
  const disabled = engine.busy || paperBusy || engine.saveError !== null
  const locked = disabled || confirmation !== null
  const action = (op: Op) => {
    if (!disabled)
      void engine
        .run(op)
        .then(() => {
          setConfirmation(null)
          if (op.type === 'new' || op.type === 'importCode' || op.type === 'restart') setPaperEpoch((value) => value + 1)
        })
        .catch(() => {})
  }
  const generate = (op: Op) => {
    if (view) setConfirmation(op)
    else action(op)
  }
  const copy = async () => {
    if (!view) return
    try {
      await host.clipboard.writeText(view.code)
      toast.success(t('code.copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  return (
    <div className="@container grid h-full min-h-0 grid-cols-[210px_minmax(0,1fr)] gap-3">
      <Generator key={view?.id ?? 'empty'} spec={view?.spec} disabled={locked} generate={generate} />
      <Panel
        title={t('title')}
        icon={<Map />}
        bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3"
        extra={view && <Badge variant={view.finished ? 'success' : 'neutral'}>{t(view.finished ? 'state.finished' : 'state.playing')}</Badge>}
      >
        <div className={locked ? 'pointer-events-none opacity-60' : ''}>
          <SegmentedControl
            size="sm"
            value={tab}
            onValueChange={setTab}
            aria-label={t('tabs.label')}
            options={[
              { value: 'play', label: t('tabs.play') },
              { value: 'paper', label: t('tabs.paper') },
            ]}
          />
        </div>
        {engine.task.error && (
          <p role="alert" className="shrink-0 text-xs text-danger">
            {errorMessage(engine.task.error)}
          </p>
        )}
        {engine.saveError && (
          <div className="shrink-0 space-y-2">
            <p role="alert" className="text-xs text-danger">
              {engine.saveError}
            </p>
            <Button size="sm" disabled={engine.busy} onClick={engine.retry}>
              {t('save.retry')}
            </Button>
          </div>
        )}
        {view ? (
          <>
            <div className="flex shrink-0 items-center gap-2">
              <div className="min-w-0 flex-1">
                <p className="truncate font-mono text-xs font-medium text-fg" title={view.code}>
                  {view.id}
                </p>
                <p className="mt-1 text-[11px] text-fg-subtle">
                  {t('stats.facts', { rows: view.spec.rows, cols: view.spec.cols, cells: view.cellCount, length: view.routeLength })}
                </p>
              </div>
              <Tooltip content={t('code.copy')}>
                <Button size="icon-sm" variant="ghost" disabled={locked} aria-label={t('code.copy')} onClick={() => void copy()}>
                  <Copy />
                </Button>
              </Tooltip>
            </div>
            {tab === 'play' ? (
              <>
                <div className="grid shrink-0 grid-cols-3 gap-2">
                  {(['steps', 'visited', 'hints'] as const).map((key) => (
                    <div key={key} className="rounded-control border border-border bg-surface-2 px-3 py-2">
                      <p className="text-[10px] text-fg-muted">{t(`stats.${key}`)}</p>
                      <p className="text-lg font-semibold tabular-nums text-fg">{view[key]}</p>
                    </div>
                  ))}
                </div>
                <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                  <Tooltip content={t('actions.undo')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      disabled={locked || view.steps === 0}
                      aria-label={t('actions.undo')}
                      onClick={() => action({ type: 'undo' })}
                    >
                      <Undo2 />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t('actions.restart')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      disabled={locked}
                      aria-label={t('actions.restart')}
                      onClick={() => setConfirmation({ type: 'restart', confirm: true })}
                    >
                      <RotateCcw />
                    </Button>
                  </Tooltip>
                  <Button size="sm" disabled={locked || view.finished} onClick={() => action({ type: 'hint' })}>
                    <Lightbulb />
                    {t('actions.hint')}
                  </Button>
                  <Button size="sm" disabled={locked} onClick={() => action({ type: 'toggleTrail' })}>
                    {view.showTrail ? <EyeOff /> : <Eye />}
                    {t(view.showTrail ? 'actions.hideTrail' : 'actions.showTrail')}
                  </Button>
                  <Button
                    size="sm"
                    disabled={locked}
                    onClick={() => (view.answerVisible ? action({ type: 'toggleAnswer' }) : setConfirmation({ type: 'toggleAnswer' }))}
                  >
                    <Flag />
                    {t(view.answerVisible ? 'actions.hideAnswer' : 'actions.answer')}
                  </Button>
                </div>
                {view.completion && (
                  <p role="status" className="shrink-0 rounded-control border border-success/30 bg-success-soft p-2 text-xs text-success">
                    {t(view.completion.hints === 0 && !view.completion.usedAnswer ? 'score.independent' : 'score.assisted', {
                      steps: view.completion.steps,
                      hints: view.completion.hints,
                    })}
                  </p>
                )}
                <div className="min-h-0 flex-1">
                  <Board view={view} disabled={locked} action={action} />
                </div>
                <p role="status" className="shrink-0 text-[11px] text-fg-muted">
                  {t(`notice.${engine.notice}`)}
                </p>
              </>
            ) : (
              <div className="min-h-0 flex-1">
                <Paper key={`${view.id}:${paperEpoch}`} disabled={disabled} source={engine.source} run={engine.run} onBusy={setPaperBusy} />
              </div>
            )}
          </>
        ) : (
          <div className="flex min-h-0 flex-1 items-center justify-center">
            <Empty icon={<Map />} title={t('empty.title')} description={t('empty.hint')} />
          </div>
        )}
        <div className="flex shrink-0 items-center gap-2 border-t border-border pt-2 text-[10px] text-fg-subtle">
          {engine.busy && <Spinner className="size-3" />}
          <span>{t(`save.${engine.status}`)}</span>
          <span className="flex-1" />
          <span>{t('algorithm')}</span>
          {engine.task.running && (
            <Button size="sm" variant="ghost" onClick={() => void engine.task.cancel().catch((error) => toast.error(errorMessage(error)))}>
              <Square />
              {t('actions.cancel')}
            </Button>
          )}
        </div>
        {engine.task.logs.length > 0 && (
          <details className="shrink-0 rounded-control border border-border">
            <summary className="cursor-pointer p-2 text-[10px] text-fg-muted">{t('logs.title')}</summary>
            <LogViewer entries={engine.task.logs} className="h-20" emptyText={t('logs.empty')} jumpLabel={t('logs.latest')} />
          </details>
        )}
      </Panel>
      <Modal
        open={confirmation !== null && engine.saveError === null}
        onOpenChange={(open) => {
          if (!open && !engine.busy) setConfirmation(null)
        }}
        title={t(confirmation?.type === 'toggleAnswer' ? 'confirm.answer' : 'confirm.replace')}
        closeLabel={t('actions.close')}
      >
        <div className="space-y-4 p-1">
          <p className="text-sm leading-relaxed text-fg-muted">{t(confirmation?.type === 'toggleAnswer' ? 'confirm.answerHint' : 'confirm.replaceHint')}</p>
          <div className="flex justify-end gap-2">
            <Button size="sm" disabled={engine.busy} onClick={() => setConfirmation(null)}>
              {t('actions.cancel')}
            </Button>
            <Button
              size="sm"
              variant="primary"
              disabled={engine.busy}
              onClick={() => {
                if (confirmation) action(confirmation)
              }}
            >
              {t('actions.confirm')}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  )
}
