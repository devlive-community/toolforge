import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Empty, Panel, Progress, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Pause, Play, RotateCcw, Timer, Trash2, Trophy } from 'lucide-react'
import { passagesFor } from './passages'
import type { Best, Lang, Mode, Score, ScoreOut, Session, Snapshot, TypingState } from './types'

const CHALLENGE_MS = 60_000

const initialState = (): TypingState => ({ cleared: {}, bests: {}, mistakes: {}, custom: '' })

const emptySession = (): Session => ({ attempts: 0, wrongAttempts: 0, pasted: false, activeMs: 0 })

/** 把 0–1 的比例格式化成百分比；无数据时显示 —，避免伪造 0% */
const pct = (value: number | null) => (value === null ? '—' : `${(value * 100).toFixed(1)}%`)
const num = (value: number | null) => (value === null ? '—' : value.toFixed(1))

/** 计算前端输入的增量：新增字符数与其中与目标位置不符的个数（口径与后端 score 一致） */
function diffInput(target: string, prev: string, next: string): { added: number; wrongAdded: number } {
  const added = Math.max(0, Array.from(next).length - Array.from(prev).length)
  if (added === 0) return { added: 0, wrongAdded: 0 }
  const nextChars = Array.from(next)
  const targetChars = Array.from(target)
  // 新增字符位于末尾 added 个位置
  const start = nextChars.length - added
  let wrongAdded = 0
  for (let i = start; i < nextChars.length; i += 1) {
    if (targetChars[i] !== nextChars[i]) wrongAdded += 1
  }
  return { added, wrongAdded }
}

export default function TypingPractice() {
  const { t, manifest, errorMessage } = usePlugin()
  const [state, setState] = usePluginState<TypingState>('state', initialState())
  const [lang, setLang] = useState<Lang>('en')
  const [mode, setMode] = useState<Mode>('free')
  const [passageIndex, setPassageIndex] = useState(0)
  const [typed, setTyped] = useState('')
  const [session, setSession] = useState<Session>(emptySession())
  const [score, setScore] = useState<Score | null>(null)
  const [running, setRunning] = useState(false)
  const [elapsed, setElapsed] = useState(0)
  const composing = useRef(false)
  const lastTyped = useRef('')
  /** 用户手动暂停后，直到再次点击继续前不自动开始计时 */
  const pausedByUser = useRef(false)
  // 连续输入时读取最新累计状态，避免闭包读到旧 session 导致尝试次数漏计
  const sessionRef = useRef<Session>(emptySession())
  // 计时以 ref 为准（计时器只写 ref 并同步到 state），避免闭包读到旧时间
  const elapsedRef = useRef(0)
  const modeRef = useRef<Mode>('free')
  useEffect(() => {
    modeRef.current = mode
  }, [mode])
  const areaRef = useRef<HTMLTextAreaElement | null>(null)

  const passages = useMemo(() => passagesFor(lang), [lang])
  const passage = passages[passageIndex % passages.length]
  // 自由与挑战模式共用同一段内置文本，区别只在于是否限时
  const target = passage.text
  const bucket = `${lang}:${mode}:${passage.id}`

  // 计时：只有有效输入且未暂停时累加；挑战模式到时由计时器回调停止（不在 effect 内 setState）
  useEffect(() => {
    if (!running) return
    const id = window.setInterval(() => {
      const next = elapsedRef.current + 100
      elapsedRef.current = next
      setElapsed(next)
      if (modeRef.current === 'challenge' && next >= CHALLENGE_MS) {
        window.clearInterval(id)
        setRunning(false)
        pausedByUser.current = false
      }
    }, 100)
    return () => window.clearInterval(id)
  }, [running])

  const reset = () => {
    setTyped('')
    lastTyped.current = ''
    sessionRef.current = emptySession()
    setSession(emptySession())
    setScore(null)
    setElapsed(0)
    elapsedRef.current = 0
    setRunning(false)
    pausedByUser.current = false
  }

  /** 把一次提交交给后端计分；组合输入（IME）期间不结算 */
  const submit = async (next: string, pasted: boolean) => {
    const prev = lastTyped.current
    const { added, wrongAdded } = diffInput(target, prev, next)
    lastTyped.current = next
    const snapshot: Snapshot = { typed: next, added, wrongAdded, pasted }
    try {
      const out = await callPlugin<ScoreOut>(manifest.id, 'score', {
        target,
        typed: snapshot.typed,
        added: snapshot.added,
        wrongAdded: snapshot.wrongAdded,
        pasted: snapshot.pasted,
        elapsedMs: elapsedRef.current,
        session: sessionRef.current,
      })
      sessionRef.current = out.session
      setSession(out.session)
      setScore(out.score)
      if (pasted) toast.info(t('typing.pasteNotice'))
      if (added > 0 && wrongAdded > 0) recordMistakes(prev, next)
      if (out.score.completed) finish(out.session, out.score)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 记录易错字符：仅统计新增位置上写错的字符 */
  const recordMistakes = (prev: string, next: string) => {
    const nextChars = Array.from(next)
    const targetChars = Array.from(target)
    const start = Array.from(prev).length
    const wrong: string[] = []
    for (let i = start; i < nextChars.length; i += 1) {
      if (targetChars[i] !== nextChars[i]) wrong.push(targetChars[i] ?? nextChars[i])
    }
    if (wrong.length === 0) return
    setState((current) => {
      const mistakes = { ...current.mistakes }
      for (const ch of wrong) mistakes[ch] = (mistakes[ch] ?? 0) + 1
      return { ...current, mistakes }
    })
  }

  /** 完成一局：粘贴过的局只展示，不写入正式纪录 */
  const finish = (finalSession: Session, final: Score) => {
    setRunning(false)
    setState((current) => {
      const cleared = { ...current.cleared, [`${lang}:${passage.id}`]: true }
      if (final.pasted) return { ...current, cleared }
      const prev = current.bests[bucket]
      const better = !prev || (final.cpm ?? 0) > (prev.cpm ?? 0)
      const best: Best | undefined = better
        ? { bucket, cpm: final.cpm, wpm: final.wpm, accuracy: final.accuracy, activeMs: finalSession.activeMs, at: new Date().toISOString() }
        : undefined
      return { ...current, cleared, bests: best ? { ...current.bests, [bucket]: best } : current.bests }
    })
    toast.success(t('typing.done'))
  }

  const onChange = (next: string) => {
    if (composing.current) {
      // 中文 IME 组合中只更新显示，不计入结果
      setTyped(next)
      return
    }
    setTyped(next)
    // 有输入即开始计时；暂停后的聚焦不会自动恢复，需用户点击继续
    if (!running && next.length > 0 && !score?.completed && !pausedByUser.current) setRunning(true)
    void submit(next, false)
  }

  const onPaste = () => {
    // 粘贴动作标记为练习模式；内容随后由 onChange 计入
    setSession((s) => ({ ...s, pasted: true }))
  }

  const onCompositionEnd = (value: string) => {
    composing.current = false
    onChange(value)
  }

  const best = state.bests[bucket]
  const challengeLeft = Math.max(0, CHALLENGE_MS - elapsed)
  const cleared = (id: string) => !!state.cleared[`${lang}:${id}`]
  const unlocked = (index: number) => index === 0 || cleared(passages[index - 1].id)
  const topMistakes = Object.entries(state.mistakes)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8)

  return (
    <div className="space-y-4 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Badge>{lang === 'zh' ? t('typing.langZh') : t('typing.langEn')}</Badge>
        <Badge>{mode === 'free' ? t('typing.modeFree') : t('typing.modeChallenge')}</Badge>
        {session.pasted && <Badge>{t('typing.practiceOnly')}</Badge>}
        <div className="ml-auto flex items-center gap-2">
          <Button size="sm" variant={lang === 'en' ? 'primary' : 'secondary'} onClick={() => { setLang('en'); setPassageIndex(0); reset() }}>
            {t('typing.langEn')}
          </Button>
          <Button size="sm" variant={lang === 'zh' ? 'primary' : 'secondary'} onClick={() => { setLang('zh'); setPassageIndex(0); reset() }}>
            {t('typing.langZh')}
          </Button>
          <Button size="sm" variant={mode === 'challenge' ? 'primary' : 'secondary'} onClick={() => { setMode(mode === 'free' ? 'challenge' : 'free'); reset() }}>
            <Timer />
            {t('typing.challenge')}
          </Button>
        </div>
      </div>

      <Panel title={t('typing.stageTitle')} bodyClassName="p-3">
        <div className="flex flex-wrap gap-2">
          {passages.map((p, index) => (
            <Button
              key={p.id}
              size="sm"
              variant={index === passageIndex ? 'primary' : 'secondary'}
              disabled={!unlocked(index)}
              onClick={() => { setPassageIndex(index); reset() }}
            >
              {t('typing.stage', { index: index + 1 })}
              {cleared(p.id) && <Trophy />}
            </Button>
          ))}
        </div>
      </Panel>

      <Panel title={t('typing.practiceTitle')} bodyClassName="space-y-3 p-3">
        <p className="rounded-card border border-border bg-surface px-3 py-2 font-mono text-[15px] leading-relaxed text-fg">
          {Array.from(target).map((ch, i) => {
            const typedChar = Array.from(typed)[i]
            const cls = typedChar === undefined ? 'text-fg-muted' : typedChar === ch ? 'text-success' : 'text-danger underline'
            return (
              <span key={i} className={cls}>
                {ch}
              </span>
            )
          })}
        </p>

        <textarea
          ref={areaRef}
          value={typed}
          onChange={(event) => onChange(event.target.value)}
          onPaste={onPaste}
          onCompositionStart={() => { composing.current = true }}
          onCompositionEnd={(event) => onCompositionEnd(event.currentTarget.value)}
          onFocus={() => undefined}
          onBlur={() => setRunning(false)}
          placeholder={t('typing.placeholder')}
          rows={3}
          spellCheck={false}
          className="w-full resize-none rounded-control border border-border bg-surface px-3 py-2 font-mono text-[15px] text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />

        <div className="flex flex-wrap items-center gap-3 text-[13px]">
          <span>{t('typing.accuracyValue', { value: pct(score?.accuracy ?? null) })}</span>
          <span>{t('typing.cpmValue', { value: num(score?.cpm ?? null) })}</span>
          {lang === 'en' && <span>{t('typing.wpmValue', { value: num(score?.wpm ?? null) })}</span>}
          <span>
            {t('typing.timeValue', { seconds: (elapsed / 1000).toFixed(1) })}
          </span>
          {mode === 'challenge' && <span>{t('typing.remaining', { seconds: (challengeLeft / 1000).toFixed(0) })}</span>}
          <div className="ml-auto flex items-center gap-2">
            <Tooltip content={running ? t('typing.pause') : t('typing.resume')}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={running ? t('typing.pause') : t('typing.resume')}
                onClick={() => {
                  pausedByUser.current = running
                  setRunning((r) => !r)
                }}
              >
                {running ? <Pause /> : <Play />}
              </Button>
            </Tooltip>
            <Tooltip content={t('typing.restart')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('typing.restart')} onClick={reset}>
                <RotateCcw />
              </Button>
            </Tooltip>
          </div>
        </div>
        <Progress value={(score?.progress ?? 0) * 100} />
      </Panel>

      <Panel title={t('typing.recordTitle')} bodyClassName="space-y-2 p-3 text-[13px]">
        <p>{best ? t('typing.bestValue', { cpm: num(best.cpm), accuracy: pct(best.accuracy) }) : t('typing.noBest')}</p>
        {topMistakes.length > 0 && (
          <p>
            {t('typing.mistakesValue', { list: topMistakes.map(([ch, n]) => `${ch}×${n}`).join('  ') })}
          </p>
        )}
        <div className="flex items-center gap-2">
          <Tooltip content={t('typing.clearRecords')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('typing.clearRecords')}
              onClick={() => setState(() => initialState())}
            >
              <Trash2 />
            </Button>
          </Tooltip>
        </div>
      </Panel>

      {passages.length === 0 && <Empty title={t('typing.empty')} />}
    </div>
  )
}
