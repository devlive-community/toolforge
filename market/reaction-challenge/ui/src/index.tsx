import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge, Button, Panel, SegmentedControl } from '@toolforge/ui'
import { usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Zap } from 'lucide-react'

interface RoundOut {
  waitMs: number
}
interface ScoreOut {
  validCount: number
  falseStarts: number
  medianMs: number | null
  official: boolean
  bestMs: number | null
  worstMs: number | null
}
interface JudgeOut {
  status: string
  errorAt: number | null
  correctLen: number
}
type Mode = 'reaction' | 'sequence'
type ReactionPhase = 'idle' | 'waiting' | 'go' | 'result' | 'yourTurn'
interface SavedRecords {
  reaction?: { median: number; at: number }
  sequence?: { length: number; at: number }
}

export default function ReactionChallenge() {
  const { t, errorMessage, call } = usePlugin()
  const [mode, setMode] = useState<Mode>('reaction')
  const [phase, setPhase] = useState<ReactionPhase>('idle')
  const [rounds, setRounds] = useState<(number | null)[]>([])
  const [score, setScore] = useState<ScoreOut | null>(null)
  const [order, setOrder] = useState<number[]>([])
  const [litIndex, setLitIndex] = useState(-1)
  const [entered, setEntered] = useState<number[]>([])
  const [judge, setJudge] = useState<JudgeOut | null>(null)
  const [level, setLevel] = useState(3)
  const [passedLevel, setPassedLevel] = useState<number | null>(null)
  const goAtRef = useRef(0)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [records, saveRecords] = usePluginState<SavedRecords>('records', {})

  const startReactionRound = useCallback(async (index: number, seedBase: number) => {
    setPhase('waiting')
    try {
      const out = await call<RoundOut>('reaction_round', { seed: seedBase + index })
      timer.current = setTimeout(() => {
        goAtRef.current = performance.now()
        setPhase('go')
      }, out.waitMs)
    } catch {
      setPhase('idle')
    }
  }, [call])

  const finishWith = useCallback(
    async (all: (number | null)[]) => {
      try {
        const out = await call<ScoreOut>('reaction_score', { timesMs: all })
        setScore(out)
        if (out.official && out.medianMs && (!records?.reaction || out.medianMs < (records.reaction?.median ?? Infinity))) {
          saveRecords({ ...records, reaction: { median: out.medianMs, at: out.medianMs * 1000 } })
        }
      } catch {
        // surfaced below
      }
    },
    [call, records, saveRecords],
  )

  const finish = useCallback(() => {
    if (rounds.length >= 5) void finishWith(rounds)
  }, [rounds, finishWith])

  const press = useCallback(() => {
    if (mode !== 'reaction') return
    if (phase === 'waiting') {
      // 抢跑
      if (timer.current) clearTimeout(timer.current)
      setRounds((prev) => [...prev, null])
      setPhase('idle')
      setTimeout(() => {
        const next = rounds.length + 1
        if (next < 5) void startReactionRound(next, Date.now() % 1_000_000)
        else finish()
      }, 600)
      return
    }
    if (phase === 'go') {
      const ms = Math.round(performance.now() - goAtRef.current)
      const nextRounds = [...rounds, ms]
      setRounds(nextRounds)
      setPhase('idle')
      if (nextRounds.length < 5) {
        setTimeout(() => void startReactionRound(nextRounds.length, Date.now() % 1_000_000), 500)
      } else {
        setRounds(nextRounds)
        setTimeout(() => void finishWith(nextRounds), 300)
      }
    }
  }, [mode, phase, rounds, startReactionRound, finishWith, finish])

  // 空格抢按
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.code === 'Space') {
        event.preventDefault()
        press()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [press])

  const startSequenceRound = async (length: number, seedBase: number) => {
    try {
      const out = await call<{ order: number[] }>('sequence', { seed: seedBase + length, length, pads: 4 })
      setOrder(out.order)
      setEntered([])
      setJudge(null)
      // 逐个亮起（600ms 一个）
      for (let index = 0; index < out.order.length; index++) {
        setTimeout(() => setLitIndex(out.order[index]), index * 600)
        setTimeout(() => setLitIndex(-1), index * 600 + 380)
      }
      setTimeout(() => setPhase('yourTurn'), out.order.length * 600 + 200)
    } catch {
      setPhase('idle')
    }
  }

  const startSequence = () => {
    setLevel(3)
    setPassedLevel(null)
    void startSequenceRound(3, Date.now() % 1_000_000)
  }


  const tapPad = async (pad: number) => {
    if (phase !== 'yourTurn') return
    const next = [...entered, pad]
    setEntered(next)
    const out = await call<JudgeOut>('sequence_judge', { expected: order, entered: next })
    setJudge(out)
    if (out.status === 'failed') {
      setPhase('result')
      if (out.correctLen >= level - 1 && !records?.sequence) {
        saveRecords({ ...records, sequence: { length: level - 1, at: level * 1000 } })
      }
    } else if (out.status === 'passed') {
      const nextLevel = level + 1
      setPassedLevel(nextLevel - 1)
      setLevel(nextLevel)
      if (nextLevel > 32) {
        setPhase('result')
        return
      }
      setTimeout(() => void startSequenceRound(nextLevel, Date.now() % 1_000_000), 700)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <SegmentedControl
        value={mode}
        onValueChange={(value) => { setMode(value as Mode); setPhase('idle'); setRounds([]); setScore(null) }}
        aria-label={t('mode.label')}
        options={[
          { value: 'reaction', label: t('mode.reaction') },
          { value: 'sequence', label: t('mode.sequence') },
        ]}
      />
      <p className="text-xs text-fg-subtle">{t('ui.hint')}</p>
      {mode === 'reaction' ? (
        <Panel icon={<Zap />} title={t('mode.reaction')} bodyClassName="space-y-3 p-3">
          <button
            type="button"
            onClick={press}
            aria-label={t('ui.start')}
            className={`flex h-32 w-full items-center justify-center rounded-card border-2 text-lg font-semibold outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
              phase === 'go'
                ? 'border-success bg-success-soft text-success'
                : phase === 'waiting'
                  ? 'border-warning bg-warning-soft text-warning'
                  : 'border-border bg-surface text-fg'
            }`}
          >
            {phase === 'go' ? t('ui.now') : phase === 'waiting' ? t('ui.wait') : t('ui.start')}
          </button>
          <div className="flex flex-wrap gap-1.5">
            {rounds.map((time, index) => (
              <Badge key={index} variant={time === null ? 'danger' : 'neutral'}>
                {time === null ? t('ui.falseStart') : `${time} ms`}
              </Badge>
            ))}
          </div>
          {score && (
            <div className="space-y-1 rounded-control border border-border p-2.5 text-xs">
              <p>
                {t('ui.median')}: {score.medianMs !== null ? `${score.medianMs} ms` : '—'}
                {score.official && <Badge variant="success" className="ml-2">{t('ui.official')}</Badge>}
              </p>
              <p className="text-fg-muted">
                {t('ui.best')}{' '}{score.bestMs ?? '—'}{' ms · '}{t('ui.falseStart')}{' '}{score.falseStarts}
              </p>
            </div>
          )}
          <Button size="sm" variant="secondary" onClick={() => { setRounds([]); setScore(null); void startReactionRound(0, Date.now() % 1_000_000) }}>
            {t('ui.playAgain')}
          </Button>
          {errorMessage && <p className="text-xs text-danger">{errorMessage({ code: 'reaction.invalid', params: {} })}</p>}
        </Panel>
      ) : (
        <Panel icon={<Zap />} title={t('mode.sequence')} bodyClassName="space-y-3 p-3">
          <p className="text-sm text-fg-muted">
            {phase === 'yourTurn' ? t('ui.yourTurn') : phase === 'idle' || phase === 'result' ? t('ui.level', { count: level }) : t('ui.watch')}
          </p>
          <div className="grid grid-cols-2 gap-2">
            {[0, 1, 2, 3].map((pad) => (
              <button
                key={pad}
                type="button"
                onClick={() => void tapPad(pad)}
                aria-label={`${pad + 1}`}
                className={`flex h-20 items-center justify-center rounded-card border-2 text-lg font-bold outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
                  litIndex === pad
                    ? 'border-primary bg-primary text-fg-on-primary'
                    : judge?.errorAt === pad
                      ? 'border-danger bg-danger-soft text-danger'
                      : 'border-border bg-surface text-fg-muted'
                }`}
              >
                {pad + 1} {['◆', '●', '▲', '■'][pad]}
              </button>
            ))}
          </div>
          {judge && judge.status === 'failed' && (
            <p className="text-xs text-danger">{t('ui.failedAt', { index: (judge.correctLen ?? 0) + 1 })}</p>
          )}
          {passedLevel !== null && <Badge variant="success">{t('ui.level', { count: passedLevel })}</Badge>}
          <Button size="sm" variant="secondary" onClick={startSequence}>
            {t('ui.playAgain')}
          </Button>
        </Panel>
      )}
      <Panel title={t('ui.records')} bodyClassName="space-y-1 p-3">
        <p className="text-xs text-fg-muted">
          {t('mode.reaction')}{' '}{records?.reaction ? `${records.reaction.median} ms` : '—'}{' · '}{t('mode.sequence')}{' '}{records?.sequence ? records.sequence.length : '—'}
        </p>
        <Button size="sm" variant="ghost" onClick={() => saveRecords({})}>
          {t('ui.clear')}
        </Button>
      </Panel>
    </div>
  )
}
