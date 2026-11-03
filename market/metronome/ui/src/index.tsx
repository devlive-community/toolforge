import { useCallback, useEffect, useRef, useState } from 'react'
import { Button, Input, Panel, Select } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Metronome as MetronomeIcon, Play, Square } from 'lucide-react'

interface Beat {
  timeMs: number
  bar: number
  beat: number
  sub: number
  level: number
}
interface ScheduleOut {
  bpm: number
  beatMs: number
  beatsPerBar: number
  subdivision: number
  beats: Beat[]
  tones: { accentHz: number; normalHz: number; subHz: number; durationMs: number }
}
interface Stage {
  bpm: number
  bars: number
}
interface PlanOut {
  stages: Stage[]
}

const SIGNATURES = ['2/4', '3/4', '4/4', '6/8']

export default function Metronome() {
  const { t } = usePlugin()
  const [bpm, setBpm] = useState(100) // tf-allow: 默认值
  const [signature, setSignature] = useState('4/4')
  const [subdivision, setSubdivision] = useState(1)
  const [countIn, setCountIn] = useState(0)
  const [volume, setVolume] = useState(60) // tf-allow: 默认值
  const [running, setRunning] = useState(false)
  const [position, setPosition] = useState<{ bar: number; beat: number; sub: number; level: number } | null>(null)
  const audioRef = useRef<AudioContext | null>(null)
  const timersRef = useRef<number[]>([])
  const [practiceStart, setPracticeStart] = useState(60) // tf-allow: 默认值
  const [practiceTarget, setPracticeTarget] = useState(80) // tf-allow: 默认值
  const [practiceStep, setPracticeStep] = useState(4) // tf-allow: 默认值
  const [savedPlan, savePlan] = usePluginState<{ start: number; target: number; step: number }>('practice', { start: 60, target: 80, step: 4 })

  const args = { bpm, signature, subdivision, bars: 64, countIn }
  const schedule = useDebouncedCall<ScheduleOut>('schedule', args, [bpm, signature, subdivision, countIn])
  const planArgs = { startBpm: practiceStart, targetBpm: practiceTarget, step: practiceStep }
  const plan = useDebouncedCall<PlanOut>('practice_plan', planArgs, [practiceStart, practiceTarget, practiceStep])

  const click = useCallback((ctx: AudioContext, atSec: number, hz: number, durationMs: number, gainValue: number) => {
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()
    osc.frequency.value = hz
    osc.type = 'square'
    gain.gain.setValueAtTime(gainValue, atSec)
    gain.gain.exponentialRampToValueAtTime(0.0001, atSec + durationMs / 1000)
    osc.connect(gain).connect(ctx.destination)
    osc.start(atSec)
    osc.stop(atSec + durationMs / 1000 + 0.02)
  }, [])

  const stopAll = useCallback(() => {
    timersRef.current.forEach((timer) => window.clearTimeout(timer))
    timersRef.current = []
    setRunning(false)
    setPosition(null)
  }, [])

  const start = () => {
    const data = schedule.result
    if (!data) return
    if (!audioRef.current) audioRef.current = new AudioContext()
    const ctx = audioRef.current
    void ctx.resume()
    const gainValue = volume / 100 * 0.5
    const startAt = ctx.currentTime + 0.1
    // 只调度前 16 小节的节拍（约每 8 秒续排一次由 playing 循环负责；这里一次性 64 小节内全部排入）
    const tones = data.tones
    for (const beat of data.beats) {
      if (beat.level === 0) continue
      const hz = beat.level === 2 ? tones.accentHz : beat.sub > 1 ? tones.subHz : tones.normalHz
      click(ctx, startAt + beat.timeMs / 1000, hz, tones.durationMs, gainValue)
      const timer = window.setTimeout(() => {
        setPosition({ bar: beat.bar, beat: beat.beat, sub: beat.sub, level: beat.level })
      }, beat.timeMs)
      timersRef.current.push(timer)
    }
    const endTimer = window.setTimeout(() => stopAll(), data.beats[data.beats.length - 1].timeMs + 500)
    timersRef.current.push(endTimer)
    setRunning(true)
  }

  // 卸载与切走时停止全部声音
  useEffect(() => () => stopAll(), [stopAll])

  const taps = useRef<number[]>([])
  const tap = () => {
    const now = Date.now()
    taps.current = [...taps.current.filter((time) => now - time < 3000), now]
    if (taps.current.length >= 2) {
      const gaps = taps.current.slice(1).map((time, index) => time - taps.current[index])
      const average = gaps.reduce((a, b) => a + b, 0) / gaps.length
      const estimated = Math.round(60000 / average)
      setBpm(Math.min(240, Math.max(30, estimated)))
    }
  }

  const beatsPerBar = schedule.result?.beatsPerBar ?? 4
  const dotPosition = position

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <Panel icon={<MetronomeIcon />} title={t('ui.bpm')} bodyClassName="space-y-3 p-3">
        <div className="flex items-center gap-3">
          <span className="w-16 font-mono text-2xl font-semibold text-fg tabular-nums">{bpm}</span>
          <div className="min-w-0 flex-1">
            <input type="range" min={30} max={240} step={1} value={bpm} onChange={(event) => setBpm(Number(event.target.value))} aria-label={t('ui.bpm')} className="w-full accent-primary" />
          </div>
          <Input type="number" value={String(bpm)} onChange={(event) => setBpm(Math.min(240, Math.max(30, Number(event.target.value) || 30)))} className="w-20" aria-label={t('ui.bpm')} />
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" variant="secondary" onClick={tap}>
            {t('ui.tap')}
          </Button>
          {running ? (
            <Button size="sm" variant="danger" onClick={stopAll}>
              <Square className="size-3.5" />
              {t('ui.stop')}
            </Button>
          ) : (
            <Button size="sm" onClick={start} disabled={!schedule.result}>
              <Play className="size-3.5" />
              {t('ui.start')}
            </Button>
          )}
          {dotPosition && (
            <span className="ml-auto font-mono text-sm text-fg tabular-nums">
              {t('ui.bar')} {dotPosition.bar} · {dotPosition.beat}.{dotPosition.sub}
            </span>
          )}
        </div>
        {/* 拍点可视化 */}
        <div className="flex gap-1.5">
          {Array.from({ length: beatsPerBar }, (_, index) => {
            const active = dotPosition?.beat === index + 1 && dotPosition.sub === 1
            const accent = index === 0
            return (
              <span
                key={index}
                className={`h-4 flex-1 rounded-control transition-colors ${
                  active ? (accent ? 'bg-danger' : 'bg-primary') : accent ? 'bg-danger-soft' : 'bg-surface-2 border border-border'
                }`}
              />
            )
          })}
        </div>
        <div className="grid grid-cols-3 gap-2">
          <label className="flex flex-col gap-1 text-xs text-fg-muted">
            {t('ui.signature')}
            <Select value={signature} onValueChange={setSignature} options={SIGNATURES.map((key) => ({ value: key, label: key }))} aria-label={t('ui.signature')} />
          </label>
          <label className="flex flex-col gap-1 text-xs text-fg-muted">
            {t('ui.subdivision')}
            <Select value={String(subdivision)} onValueChange={(value) => setSubdivision(Number(value))} options={[1, 2, 3, 4].map((value) => ({ value: String(value), label: `${value}` }))} aria-label={t('ui.subdivision')} />
          </label>
          <label className="flex flex-col gap-1 text-xs text-fg-muted">
            {t('ui.countIn')}
            <Select value={String(countIn)} onValueChange={(value) => setCountIn(Number(value))} options={[0, 1, 2, 3, 4].map((value) => ({ value: String(value), label: `${value}` }))} aria-label={t('ui.countIn')} />
          </label>
        </div>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.volume')}
          <div className="min-w-0 flex-1">
            <input type="range" min={0} max={100} step={1} value={volume} onChange={(event) => setVolume(Number(event.target.value))} aria-label={t('ui.volume')} className="w-full accent-primary" />
          </div>
        </label>
      </Panel>
      <Panel icon={<MetronomeIcon />} title={t('ui.practice')} bodyClassName="space-y-2.5 p-3">
        <div className="grid grid-cols-3 gap-2">
          <Input type="number" value={String(practiceStart)} onChange={(event) => setPracticeStart(Number(event.target.value) || 30)} placeholder={t('api.practice_plan.startBpm')} aria-label={t('api.practice_plan.startBpm')} />
          <Input type="number" value={String(practiceTarget)} onChange={(event) => setPracticeTarget(Number(event.target.value) || 30)} placeholder={t('api.practice_plan.targetBpm')} aria-label={t('api.practice_plan.targetBpm')} />
          <Input type="number" value={String(practiceStep)} onChange={(event) => setPracticeStep(Number(event.target.value) || 1)} placeholder={t('api.practice_plan.step')} aria-label={t('api.practice_plan.step')} />
        </div>
        {plan.result && (
          <>
            <div className="flex flex-wrap gap-1.5">
              {plan.result.stages.map((stage) => (
                <button
                  key={stage.bpm}
                  type="button"
                  onClick={() => setBpm(Math.min(240, Math.max(30, Math.round(stage.bpm))))}
                  className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {stage.bpm}{' × '}{stage.bars}
                </button>
              ))}
            </div>
            <Button size="sm" variant="secondary" onClick={() => savePlan({ start: practiceStart, target: practiceTarget, step: practiceStep })}>
              {t('ui.apply')}
            </Button>
          </>
        )}
        {savedPlan && (
          <p className="text-[11px] text-fg-subtle">
            {t('ui.everyBars', { count: 4, step: savedPlan.step })}: {savedPlan.start}{' → '}{savedPlan.target}
          </p>
        )}
      </Panel>
    </div>
  )
}
