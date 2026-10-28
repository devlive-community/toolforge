import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Empty, Panel, Progress, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Copy, Pause, Play, RotateCcw, Timer, Trash2, Trophy } from 'lucide-react'
import { passagesFor } from './passages'
import { createQueue, createSettler, mergeMistakes, pasteFlag, type Settlement } from './settle'
import type { Lang, Mode, Prepared, Score, ScoreOut, Session, SettleOut, TypingState } from './types'

const CHALLENGE_MS = 60_000
/** 计时器两次触发之间最多计入的时长：系统休眠或节流后的长间隔不算有效输入时间 */
const MAX_TICK_MS = 1_000

const initialState = (): TypingState => ({ cleared: {}, bests: {}, mistakes: {}, custom: '' })

const emptySession = (): Session => ({ attempts: 0, wrongAttempts: 0, pasted: false, activeMs: 0 })

/** 把 0–1 的比例格式化成百分比；无数据时显示 —，避免伪造 0% */
const pct = (value: number | null) => (value === null ? '—' : `${(value * 100).toFixed(1)}%`)
const num = (value: number | null) => (value === null ? '—' : value.toFixed(1))

/** 一局的完整结果：后端成绩 + 用时 + 是否粘贴（结算器据此决定是否记录） */
type RoundResult = Score & { activeMs: number; pasted: boolean }
/** 本局结束的原因：完成全文或挑战到点 */
type Finished = 'done' | 'timeUp' | null

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
  const [finished, setFinished] = useState<Finished>(null)
  const [confirmClear, setConfirmClear] = useState(false)
  const composing = useRef(false)
  /** 最近一次交给后端的文本（提交时同步更新，作为下一次的 prev） */
  const lastTyped = useRef('')
  // 连续输入时读取最新累计状态，避免闭包读到旧 session 导致尝试次数漏计
  const sessionRef = useRef<Session>(emptySession())
  /** 最近一次后端返回的成绩，挑战到点时用它结算 */
  const latestScore = useRef<Score | null>(null)
  /** 局号：重新开始后递增，旧局迟到的后端结果直接丢弃 */
  const roundRef = useRef(0)
  /** 粘贴/拖放事件先于 onChange 发生：置位后由下一次提交消费并计入本局 */
  const pendingPaste = useRef(false)
  /** 一局只结算一次（规则与测试见 settle.ts） */
  const [settler] = useState(() => createSettler())
  /** 提交串行执行，保证每次都基于上一次返回的累计状态 */
  const [queue] = useState(() => createQueue())
  // 计时以 ref 为准（计时器只写 ref 并同步到 state），避免闭包读到旧时间
  const elapsedRef = useRef(0)
  const modeRef = useRef<Mode>('free')
  const stateRef = useRef(state)
  useEffect(() => {
    modeRef.current = mode
    stateRef.current = state
  }, [mode, state])
  const areaRef = useRef<HTMLTextAreaElement | null>(null)
  /** 计时器通过 ref 调用当前渲染的挑战到点结算（effect 中写 ref，渲染期写 ref 会被 lint 拦截） */
  const settleTimeUpRef = useRef<(() => void) | null>(null)

  const passages = useMemo(() => passagesFor(lang), [lang])
  const passage = passages[passageIndex % passages.length]
  // 内容来源：内置关卡与自定义文本分开比较、分开解锁
  const [source, setSource] = useState<'builtin' | 'custom'>('builtin')
  const customText = state.custom.trim()
  // 自由与挑战模式共用同一段文本，区别只在于是否限时
  const target = source === 'custom' ? customText : passage.text

  /** 当前文本的准备结果（Rust 切分可见字符、计算指纹并检查长度）；文本变了就重新准备 */
  const [preparedFor, setPreparedFor] = useState<{ text: string; value: Prepared | null; error: string | null } | null>(null)
  const ready = preparedFor !== null && preparedFor.text === target ? preparedFor : null
  const prepared = ready?.value ?? null
  useEffect(() => {
    if (!target) return
    let alive = true
    callPlugin<Prepared>(manifest.id, 'prepare', { text: target })
      .then((value) => {
        if (alive) setPreparedFor({ text: target, value, error: null })
      })
      .catch((error) => {
        if (alive) setPreparedFor({ text: target, value: null, error: errorMessage(error) })
      })
    return () => {
      alive = false
    }
  }, [manifest.id, target, errorMessage])

  // 纪录桶：来源 + 语言 + 模式 + 内容 + 内容指纹，文本改动后旧纪录不再比较
  const bucket = prepared
    ? `${source}:${lang}:${mode}:${source === 'custom' ? 'custom' : passage.id}:${prepared.fingerprint}`
    : null
  const stageKey = source === 'builtin' ? `${lang}:${passage.id}` : null
  // 开局时固定纪录桶与关卡：完成时写入开局的那一桶
  const roundBucket = useRef<{ bucket: string; stage: string | null } | null>(null)

  // 计时：有效输入后才运行；按真实流逝时间累加，挑战模式到点由计时器回调结束
  useEffect(() => {
    if (!running) return
    let last = performance.now()
    const id = window.setInterval(() => {
      const now = performance.now()
      const delta = Math.min(MAX_TICK_MS, now - last)
      last = now
      const challenge = modeRef.current === 'challenge'
      const next = challenge ? Math.min(CHALLENGE_MS, elapsedRef.current + delta) : elapsedRef.current + delta
      elapsedRef.current = next
      setElapsed(next)
      if (challenge && next >= CHALLENGE_MS) {
        window.clearInterval(id)
        setRunning(false)
        settleTimeUpRef.current?.()
      }
    }, 100)
    return () => window.clearInterval(id)
  }, [running])

  // 窗口切到后台时暂停计时（输入框失焦另由 onBlur 处理）
  useEffect(() => {
    const onHidden = () => {
      if (document.hidden) setRunning(false)
    }
    document.addEventListener('visibilitychange', onHidden)
    return () => document.removeEventListener('visibilitychange', onHidden)
  }, [])

  const reset = () => {
    roundRef.current += 1
    latestScore.current = null
    roundBucket.current = null
    setTyped('')
    lastTyped.current = ''
    sessionRef.current = emptySession()
    setSession(emptySession())
    setScore(null)
    setElapsed(0)
    elapsedRef.current = 0
    setRunning(false)
    setFinished(null)
    pendingPaste.current = false
    settler.reset()
  }

  /** 一局结束：交给 Rust 判断是否写纪录、是否刷新最好、是否解锁关卡 */
  const finish = (result: Settlement<RoundResult>, why: Exclude<Finished, null>) => {
    setRunning(false)
    setFinished(why)
    const round = roundRef.current
    const meta = roundBucket.current
    if (!meta) return
    callPlugin<SettleOut>(manifest.id, 'settle', {
      bests: stateRef.current.bests,
      bucket: meta.bucket,
      stage: meta.stage !== null,
      round: {
        cpm: result.cpm,
        wpm: result.wpm,
        accuracy: result.accuracy,
        activeMs: result.activeMs,
        passed: result.passed,
        pasted: !result.record,
      },
      at: new Date().toISOString(),
    })
      .then((out) => {
        if (round !== roundRef.current) return
        const stage = meta.stage
        setState((current) => ({
          ...current,
          bests: out.bests,
          cleared: out.cleared && stage ? { ...current.cleared, [stage]: true } : current.cleared,
        }))
        if (why === 'timeUp') toast.info(t('typing.timeUp'))
        if (out.cleared) toast.success(t('typing.stagePassed'))
        else if (meta.stage && result.completed && !result.passed) toast.info(t('typing.stageFailed'))
        else if (why === 'done') toast.success(t('typing.done'))
        if (out.newBest) toast.success(t('typing.newBest'))
      })
      .catch((error) => toast.error(errorMessage(error)))
  }

  /** 把一次提交交给后端计分（串行）；IME 组合期间不会走到这里 */
  const submit = (next: string, pasted: boolean) => {
    const prev = lastTyped.current
    lastTyped.current = next
    const round = roundRef.current
    void queue.push(async () => {
      if (round !== roundRef.current) return
      try {
        const out = await callPlugin<ScoreOut>(manifest.id, 'score', {
          target,
          prev,
          typed: next,
          pasted,
          elapsedMs: Math.round(elapsedRef.current),
          session: sessionRef.current,
        })
        if (round !== roundRef.current) return
        // 本局一旦出现过粘贴，即使后续继续手打，也保持练习标记
        const carried = { ...out.session, pasted: pasteFlag(sessionRef.current.pasted, out.session.pasted) }
        sessionRef.current = carried
        latestScore.current = out.score
        setSession(carried)
        setScore(out.score)
        if (pasted) toast.info(t('typing.pasteNotice'))
        if (out.score.missed.length > 0) {
          setState((current) => ({ ...current, mistakes: mergeMistakes(current.mistakes, out.score.missed) }))
        }
        if (out.score.completed) {
          const result: RoundResult = { ...out.score, activeMs: carried.activeMs, pasted: carried.pasted }
          settler.complete(result, (r) => finish(r, 'done'))
        }
      } catch (error) {
        toast.error(errorMessage(error))
      }
    })
  }

  /** 挑战到点：等排队中的提交算完，再用最后的成绩结算（无输入不结算） */
  const settleTimeUp = () => {
    setFinished('timeUp')
    const round = roundRef.current
    void queue.push(async () => {
      const last = latestScore.current
      if (round !== roundRef.current || !last || last.total === 0) return
      const result: RoundResult = { ...last, activeMs: Math.round(elapsedRef.current), pasted: sessionRef.current.pasted }
      settler.complete(result, (r) => finish(r, 'timeUp'))
    })
  }
  useEffect(() => {
    settleTimeUpRef.current = settleTimeUp
  })

  const onChange = (next: string) => {
    setTyped(next)
    // 中文 IME 组合中只更新显示，不计入结果
    if (composing.current || finished || !bucket) return
    // 同一文本重复上报（如组合结束与 input 事件各一次）不重复提交
    if (next === lastTyped.current) return
    // 第一次输入时固定本局的纪录桶与关卡
    if (!roundBucket.current) roundBucket.current = { bucket, stage: stageKey }
    // 有提交即开始/恢复计时；暂停或失焦后，再次输入才继续
    if (!running) setRunning(true)
    const pasted = pendingPaste.current
    pendingPaste.current = false
    submit(next, pasted)
  }

  /** 易错字符重练：由 Rust 生成只含这些字符的文本，切换为自定义来源开始练习 */
  const drillMistakes = async () => {
    const chars = Object.keys(state.mistakes).join('')
    if (!chars) return
    try {
      const out = await callPlugin<{ text: string; length: number }>(manifest.id, 'drill', {
        chars,
        len: 200,
        seed: Date.now() % 1_000_000,
      })
      setState((current) => ({ ...current, custom: out.text }))
      setSource('custom')
      reset()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 导出练习摘要（纯文本复制到剪贴板，不进入 Agent） */
  const exportSummary = async () => {
    const lines = [
      `${t('typing.summaryTitle')}`,
      ...Object.values(state.bests).map((b) => `${b.bucket}: CPM ${num(b.cpm)} · ${t('typing.accuracy')} ${pct(b.accuracy)}`),
      topMistakes.length > 0 ? `${t('typing.mistakes')}: ${topMistakes.map(([ch, n]) => `${ch}×${n}`).join(' ')}` : '',
    ].filter(Boolean)
    try {
      await host.clipboard.writeText(lines.join('\n'))
      toast.success(t('typing.summaryCopied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 清空纪录：第一次点击进入确认，再点一次才清空；自定义文本保留 */
  const clearRecords = () => {
    if (!confirmClear) {
      setConfirmClear(true)
      return
    }
    setConfirmClear(false)
    setState((current) => ({ ...initialState(), custom: current.custom }))
    toast.success(t('typing.cleared'))
  }

  const markPasted = () => {
    // 粘贴或拖入文字标记为练习局；内容随后由 onChange 计入本局
    pendingPaste.current = true
    sessionRef.current = { ...sessionRef.current, pasted: true }
    setSession((s) => ({ ...s, pasted: true }))
  }

  const onCompositionEnd = (value: string) => {
    composing.current = false
    onChange(value)
  }

  const best = bucket ? state.bests[bucket] : undefined
  const challengeLeft = Math.max(0, CHALLENGE_MS - elapsed)
  const cleared = (id: string) => !!state.cleared[`${lang}:${id}`]
  const unlocked = (index: number) => index === 0 || cleared(passages[index - 1].id)
  const topMistakes = Object.entries(state.mistakes)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8)
  const marks = score?.marks ?? []
  const extra = Math.max(0, marks.length - (prepared?.total ?? 0))
  const inputDisabled = !prepared || finished !== null

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

      <Panel title={t('typing.sourceTitle')} bodyClassName="space-y-2 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button size="sm" variant={source === 'builtin' ? 'primary' : 'secondary'} onClick={() => { setSource('builtin'); reset() }}>
            {t('typing.sourceBuiltin')}
          </Button>
          <Button size="sm" variant={source === 'custom' ? 'primary' : 'secondary'} onClick={() => { setSource('custom'); reset() }}>
            {t('typing.sourceCustom')}
          </Button>
        </div>
        {source === 'custom' && (
          <>
            <textarea
              value={state.custom}
              onChange={(event) => {
                // 练习文本变了，本局作废重来
                reset()
                const value = event.target.value
                setState((current) => ({ ...current, custom: value }))
              }}
              placeholder={t('typing.customPlaceholder')}
              rows={3}
              className="w-full resize-none rounded-control border border-border bg-surface px-3 py-2 text-[14px] text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            {ready?.error && <p className="text-[12px] text-danger">{ready.error}</p>}
            {prepared && (
              <p className="text-[12px] text-fg-muted">{t('typing.customLimit', { count: prepared.total, max: prepared.maxText })}</p>
            )}
          </>
        )}
      </Panel>

      {source === 'builtin' && (
        <Panel title={t('typing.stageTitle')} bodyClassName="space-y-2 p-3">
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
          {prepared && <p className="text-[12px] text-fg-muted">{t('typing.stageGoal', { accuracy: pct(prepared.passAccuracy) })}</p>}
        </Panel>
      )}

      <Panel title={t('typing.practiceTitle')} bodyClassName="space-y-3 p-3">
        <p className="whitespace-pre-wrap break-words rounded-card border border-border bg-surface px-3 py-2 font-mono text-[15px] leading-relaxed text-fg">
          {prepared
            ? prepared.graphemes.map((ch, i) => {
                const mark = marks[i]
                const cls =
                  mark === undefined
                    ? i === marks.length && !finished
                      ? 'bg-active text-fg underline'
                      : 'text-fg-muted'
                    : mark
                      ? 'text-success'
                      : 'bg-danger-soft text-danger underline'
                return (
                  <span key={i} className={cls}>
                    {ch === '\n' ? '↵\n' : ch}
                  </span>
                )
              })
            : target
              ? (ready?.error ?? t('typing.preparing'))
              : t('typing.customPlaceholder')}
          {extra > 0 && <span className="bg-danger-soft text-danger">{'·'.repeat(Math.min(extra, 50))}</span>}
        </p>

        <textarea
          ref={areaRef}
          value={typed}
          onChange={(event) => onChange(event.target.value)}
          disabled={inputDisabled}
          onPaste={markPasted}
          onDrop={markPasted}
          onCompositionStart={() => { composing.current = true }}
          onCompositionEnd={(event) => onCompositionEnd(event.currentTarget.value)}
          onBlur={() => setRunning(false)}
          placeholder={finished ? t('typing.finished') : t('typing.placeholder')}
          rows={3}
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          className="w-full resize-none rounded-control border border-border bg-surface px-3 py-2 font-mono text-[15px] text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60"
        />

        <div className="flex flex-wrap items-center gap-3 text-[13px]">
          <span>{t('typing.accuracyValue', { value: pct(score?.accuracy ?? null) })}</span>
          <span>{t('typing.cpmValue', { value: num(score?.cpm ?? null) })}</span>
          {lang === 'en' && <span>{t('typing.wpmValue', { value: num(score?.wpm ?? null) })}</span>}
          <span>{t('typing.wrongAttemptsValue', { value: session.wrongAttempts })}</span>
          <span>{t('typing.wrongNowValue', { value: score?.wrongNow ?? 0 })}</span>
          <span>{t('typing.timeValue', { seconds: (elapsed / 1000).toFixed(1) })}</span>
          {mode === 'challenge' && <span>{t('typing.remaining', { seconds: Math.ceil(challengeLeft / 1000) })}</span>}
          <div className="ml-auto flex items-center gap-2">
            <Tooltip content={running ? t('typing.pause') : t('typing.resume')}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={running ? t('typing.pause') : t('typing.resume')}
                disabled={finished !== null || session.attempts === 0}
                onClick={() => {
                  // 暂停立即停表；继续则回到输入框，下一次输入时接着计时
                  if (running) setRunning(false)
                  else areaRef.current?.focus()
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
          <>
            <p>{t('typing.mistakesValue', { list: topMistakes.map(([ch, n]) => `${ch}×${n}`).join('  ') })}</p>
            <div className="flex flex-wrap items-center gap-2">
              <Button size="sm" variant="secondary" onClick={drillMistakes}>
                {t('typing.drillMistakes')}
              </Button>
            </div>
          </>
        )}
        <div className="flex items-center gap-2">
          <Tooltip content={t('typing.exportSummary')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('typing.exportSummary')} onClick={exportSummary}>
              <Copy />
            </Button>
          </Tooltip>
          <Tooltip content={confirmClear ? t('typing.clearConfirm') : t('typing.clearRecords')}>
            <Button
              size="icon-sm"
              variant={confirmClear ? 'danger' : 'ghost'}
              aria-label={t('typing.clearRecords')}
              onClick={clearRecords}
              onBlur={() => setConfirmClear(false)}
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
