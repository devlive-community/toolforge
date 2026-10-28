import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Empty, Panel, Progress, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Copy, Pause, Play, RotateCcw, Timer, Trash2, Trophy } from 'lucide-react'
import { passagesFor } from './passages'
import { createSettler, pasteFlag, type Settlement } from './settle'
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

/** 一局的完整结果：后端成绩 + 用时 + 是否粘贴（结算器据此决定是否记录） */
type RoundResult = Score & { activeMs: number; pasted: boolean }

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
  /** 粘贴事件先于 onChange 发生：粘贴后置位，由下一次提交消费并计入本局 */
  const pendingPaste = useRef(false)
  /** 一局只结算一次（规则与测试见 settle.ts）；处理函数在 complete 时传入，始终是当前渲染的 finish */
  const [settler] = useState(() => createSettler())
  // 计时以 ref 为准（计时器只写 ref 并同步到 state），避免闭包读到旧时间
  const elapsedRef = useRef(0)
  const modeRef = useRef<Mode>('free')
  useEffect(() => {
    modeRef.current = mode
  }, [mode])
  const areaRef = useRef<HTMLTextAreaElement | null>(null)

  const passages = useMemo(() => passagesFor(lang), [lang])
  const passage = passages[passageIndex % passages.length]
  // 内容来源：内置关卡与自定义文本分开比较、分开解锁
  const [source, setSource] = useState<'builtin' | 'custom'>('builtin')
  const customText = state.custom.trim()
  // 自由与挑战模式共用同一段文本，区别只在于是否限时
  const target = source === 'custom' ? customText : passage.text
  const bucket = `${source}:${lang}:${mode}:${source === 'custom' ? 'custom' : passage.id}`
  // 开局时固定纪录桶：完成时即使用户已切换语言/模式，也写入开局的那一桶
  const sessionBucket = useRef(bucket)

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
    pendingPaste.current = false
    settler.reset()
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
      // 本局一旦出现过粘贴，即使后续继续手打，也保持练习标记
      const carried = { ...out.session, pasted: pasteFlag(sessionRef.current.pasted, out.session.pasted) }
      sessionRef.current = carried
      setSession(carried)
      setScore(out.score)
      if (pasted) toast.info(t('typing.pasteNotice'))
      if (added > 0 && wrongAdded > 0) recordMistakes(prev, next)
      if (out.score.completed) {
        const round: RoundResult = { ...out.score, activeMs: carried.activeMs, pasted: carried.pasted }
        settler.complete(round, finish)
      }
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

  /** 完成一局：粘贴过的局只展示，不写入正式纪录（record 由结算器给出） */
  const finish = (result: Settlement<RoundResult>) => {
    setRunning(false)
    const key = sessionBucket.current
    const isBuiltin = key.startsWith('builtin:')
    setState((current) => {
      // 只有内置关卡参与解锁；自定义文本不影响关卡进度
      const cleared = isBuiltin ? { ...current.cleared, [`${lang}:${passage.id}`]: true } : current.cleared
      if (!result.record) return { ...current, cleared }
      const prev = current.bests[key]
      const better = !prev || (result.cpm ?? 0) > (prev.cpm ?? 0)
      const best: Best | undefined = better
        ? { bucket: key, cpm: result.cpm, wpm: result.wpm, accuracy: result.accuracy, activeMs: result.activeMs, at: new Date().toISOString() }
        : undefined
      return { ...current, cleared, bests: best ? { ...current.bests, [key]: best } : current.bests }
    })
    toast.success(t('typing.done'))
  }

  const onChange = (next: string) => {
    if (composing.current) {
      // 中文 IME 组合中只更新显示，不计入结果
      setTyped(next)
      return
    }
    // 第一次输入时固定本局的纪录桶
    if (lastTyped.current === '' && next.length > 0) sessionBucket.current = bucket
    setTyped(next)
    // 有输入即开始计时；暂停后的聚焦不会自动恢复，需用户点击继续
    if (!running && next.length > 0 && !score?.completed && !pausedByUser.current) setRunning(true)
    const pasted = pendingPaste.current
    pendingPaste.current = false
    void submit(next, pasted)
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

  const onPaste = () => {
    // 粘贴动作标记为练习模式；内容随后由 onChange 计入本局
    pendingPaste.current = true
    sessionRef.current = { ...sessionRef.current, pasted: true }
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
          <textarea
            value={state.custom}
            onChange={(event) => setState((current) => ({ ...current, custom: event.target.value }))}
            placeholder={t('typing.customPlaceholder')}
            rows={3}
            className="w-full resize-none rounded-control border border-border bg-surface px-3 py-2 text-[14px] text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        )}
      </Panel>

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
          disabled={source === 'custom' && customText.length === 0}
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
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" variant="secondary" onClick={drillMistakes}>
              {t('typing.drillMistakes')}
            </Button>
          </div>
        )}
        {topMistakes.length > 0 && (
          <p>
            {t('typing.mistakesValue', { list: topMistakes.map(([ch, n]) => `${ch}×${n}`).join('  ') })}
          </p>
        )}
        <div className="flex items-center gap-2">
          <Tooltip content={t('typing.exportSummary')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('typing.exportSummary')} onClick={exportSummary}>
              <Copy />
            </Button>
          </Tooltip>
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
