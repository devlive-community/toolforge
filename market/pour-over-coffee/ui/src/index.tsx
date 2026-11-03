import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Input, Panel, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Coffee, Copy, Timer, Trash2 } from 'lucide-react'

interface Stage {
  label: string
  targetMl: number
  cumulativeMl: number
  seconds?: number
}
interface PlanOut {
  coffeeG: number
  ratio: number
  totalMl: number
  covered: number
  stages: Stage[]
}
interface BrewNote {
  id: number
  bean: string
  roast: string
  grind: string
  temp: string
  taste: string
  score: number
  coffeeG: number
  ratio: number
  stageMl: number[]
  date: number
}

export default function PourOverCoffee() {
  const { t, errorMessage } = usePlugin()
  const [coffee, setCoffee] = useState(15) // tf-allow: 默认值
  const [ratio, setRatio] = useState(15) // tf-allow: 默认值
  const [stageText, setStageText] = useState('0.15 闷蒸, 0.4 二段, 0.45 三段') // tf-allow: 默认值

  const stages = stageText
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part, index) => {
      const [num, ...rest] = part.split(/\s+/)
      const value = Number(num)
      if (!Number.isFinite(value)) return null
      return rest.length > 0 ? { fraction: value, label: rest.join(' ') } : { fraction: value, label: `${index + 1}` }
    })
    .filter((s): s is { fraction: number; label: string } => s !== null)
  const args = coffee > 0 && ratio > 0 && stages.length > 0 ? { coffeeG: coffee, ratio, stages } : null
  const plan = useDebouncedCall<PlanOut>(args ? 'plan' : null, args, [coffee, ratio, stageText])

  // 现场步骤：计时 + 当前段
  const [running, setRunning] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const [stepIndex, setStepIndex] = useState(0)
  const timer = useRef<ReturnType<typeof setInterval> | null>(null)
  useEffect(() => {
    if (running) {
      timer.current = setInterval(() => setElapsed((prev) => prev + 1), 1000)
    }
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [running])

  const current = plan.result?.stages[stepIndex]
  const advance = () => {
    if (plan.result && stepIndex < plan.result.stages.length - 1) setStepIndex((prev) => prev + 1)
    else setRunning(false)
  }

  // 杯次记录
  const [notes, saveNotes] = usePluginState<BrewNote[]>('brews', [])
  const [noteDraft, setNoteDraft] = useState({ bean: '', roast: '', grind: '', temp: '92', taste: '', score: 4 }) // tf-allow: 默认值

  const saveBrew = () => {
    const note: BrewNote = {
      id: Date.now(),
      bean: noteDraft.bean.trim(),
      roast: noteDraft.roast.trim(),
      grind: noteDraft.grind.trim(),
      temp: noteDraft.temp.trim(),
      taste: noteDraft.taste.trim(),
      score: noteDraft.score,
      coffeeG: coffee,
      ratio,
      stageMl: plan.result?.stages.map((s) => s.targetMl) ?? [],
      date: Date.now(),
    }
    saveNotes([note, ...(notes ?? [])])
  }

  const copyAsRecipe = (note: BrewNote) => {
    setCoffee(note.coffeeG)
    setRatio(note.ratio)
    setStageText(note.stageMl.map((ml) => `${ml} ml`).join(', '))
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Coffee />} title={t('plan.title')} bodyClassName="space-y-2.5 p-3">
          <div className="grid grid-cols-2 gap-2">
            <Input type="number" value={String(coffee)} onChange={(event) => setCoffee(Number(event.target.value))} placeholder={t('api.plan.coffeeG')} aria-label={t('api.plan.coffeeG')} />
            <Input type="number" value={String(ratio)} onChange={(event) => setRatio(Number(event.target.value))} placeholder={t('api.plan.ratio')} aria-label={t('api.plan.ratio')} />
          </div>
          <Input value={stageText} onChange={(event) => setStageText(event.target.value)} placeholder={t('scale.stageMlHint')} aria-label={t('scale.stageMlHint')} />
          <p className="text-[11px] text-fg-subtle">{t('ui.stageHint')}</p>
          {plan.error ? (
            <p className="text-xs text-danger">{errorMessage(plan.error)}</p>
          ) : plan.result ? (
            <div className="space-y-1">
              <ValueRow label={t('plan.total')} value={`${plan.result.totalMl} ml`} labelWidth="w-24" />
              <ValueRow label={t('plan.covered')} value={`${Math.round(plan.result.covered * 100)}%`} labelWidth="w-24" />
            </div>
          ) : (
            <p className="text-xs text-fg-subtle">{t('plan.title')}</p>
          )}
        </Panel>
        <Panel icon={<Timer />} title={t('brew.step')} bodyClassName="space-y-2 p-3" extra={running && <Spinner className="size-3.5" />}>
          {current ? (
            <>
              <p className="text-lg font-semibold text-fg">
                {current.label}{' · +'}{current.targetMl}{' ml'}
              </p>
              <ValueRow label={t('plan.cumulative')} value={`${current.cumulativeMl} ml`} labelWidth="w-24" />
              <ValueRow label={t('brew.elapsed', { value: elapsed })} value="" labelWidth="w-24" />
              <div className="flex gap-2">
                <Button size="sm" variant="secondary" onClick={() => setRunning((prev) => !prev)}>
                  {running ? t('brew.pause') : t('brew.start')}
                </Button>
                <Button size="sm" variant="secondary" onClick={advance}>
                  {t('brew.next')}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => { setElapsed(0); setStepIndex(0); setRunning(false) }}>
                  {t('brew.reset')}
                </Button>
              </div>
            </>
          ) : (
            <p className="text-xs text-fg-subtle">{t('ui.stageHint')}</p>
          )}
        </Panel>
      </div>
      <Panel icon={<Coffee />} title={t('notes.title')} className="min-h-0" bodyClassName="min-h-0 overflow-auto space-y-2 p-3">
        <div className="grid grid-cols-2 gap-2">
          <Input size="sm" value={noteDraft.bean} onChange={(event) => setNoteDraft((prev) => ({ ...prev, bean: event.target.value }))} placeholder={t('notes.bean')} aria-label={t('notes.bean')} />
          <Input size="sm" value={noteDraft.roast} onChange={(event) => setNoteDraft((prev) => ({ ...prev, roast: event.target.value }))} placeholder={t('notes.roast')} aria-label={t('notes.roast')} />
          <Input size="sm" value={noteDraft.grind} onChange={(event) => setNoteDraft((prev) => ({ ...prev, grind: event.target.value }))} placeholder={t('notes.grind')} aria-label={t('notes.grind')} />
          <Input size="sm" value={noteDraft.temp} onChange={(event) => setNoteDraft((prev) => ({ ...prev, temp: event.target.value }))} placeholder={t('notes.temp')} aria-label={t('notes.temp')} />
          <Input size="sm" value={noteDraft.taste} onChange={(event) => setNoteDraft((prev) => ({ ...prev, taste: event.target.value }))} placeholder={t('notes.taste')} aria-label={t('notes.taste')} />
          <Input size="sm" type="number" value={String(noteDraft.score)} onChange={(event) => setNoteDraft((prev) => ({ ...prev, score: Number(event.target.value) }))} placeholder={t('notes.score')} aria-label={t('notes.score')} />
        </div>
        <Button size="sm" variant="secondary" onClick={saveBrew}>
          {t('notes.save')}
        </Button>
        {(notes ?? []).map((note) => (
          <div key={note.id} className="rounded-control border border-border p-2.5">
            <div className="flex items-center justify-between">
              <span className="text-[13px] font-medium text-fg">{note.bean || t('notes.bean')}</span>
              <Badge variant="neutral">{t('notes.score')}: {note.score}</Badge>
            </div>
            <p className="mt-0.5 text-[11px] text-fg-muted">
              {note.coffeeG}{'g · 1:'}{note.ratio}{' · '}{note.grind}{' · '}{note.temp}{'°C'}
            </p>
            {note.taste && <p className="mt-1 text-xs text-fg-muted">{note.taste}</p>}
            <div className="mt-1.5 flex gap-3">
              <button type="button" onClick={() => copyAsRecipe(note)} className="flex items-center gap-1 text-[11px] text-primary-fg outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <Copy className="size-3" />
                {t('notes.again')}
              </button>
              <button
                type="button"
                aria-label={t('notes.delete')}
                onClick={() => saveNotes((notes ?? []).filter((n) => n.id !== note.id))}
                className="text-fg-subtle outline-none hover:text-danger focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Trash2 className="size-3" />
              </button>
            </div>
          </div>
        ))}
        {(!notes || notes.length === 0) && <p className="text-xs text-fg-subtle">{t('notes.empty')}</p>}
      </Panel>
    </div>
  )
}
