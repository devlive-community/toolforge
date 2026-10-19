import { useMemo, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Progress, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import {
  BookOpen,
  CalendarClock,
  Check,
  Download,
  Eye,
  EyeOff,
  Layers,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  Upload,
  X,
} from 'lucide-react'
import type {
  ApplyOut,
  Card,
  ExportOut,
  FlashcardsState,
  ImportOut,
  PreviewOut,
  QueueOut,
  ReviewLogEntry,
  ReviewOut,
} from './types'
import { toBackupCard, toCard } from './types'

/** 本地日历日（YYYY-MM-DD），调度日期口径以此为准 */
function today(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${month}-${day}`
}

const initialState = (): FlashcardsState => ({
  cards: [],
  sessionLimit: 20,
  newLimit: 10,
  lastUndo: null,
})

type View = { kind: 'review' } | { kind: 'deck'; name: string } | { kind: 'import' } | { kind: 'backup' }

export default function Flashcards() {
  const { t, errorMessage, manifest } = usePlugin()
  const [state, setState] = usePluginState<FlashcardsState>('state', initialState())
  const [view, setView] = useState<View>({ kind: 'review' })
  const today_ = today()

  // ---- 复习会话（内存态，不持久化）----
  const [queue, setQueue] = useState<QueueOut | null>(null)
  const [queueIndex, setQueueIndex] = useState(0)
  const [revealed, setRevealed] = useState(false)
  const [sessionDone, setSessionDone] = useState(false)
  const [previewRemember, setPreviewRemember] = useState<ReviewOut | null>(null)
  const [previewForget, setPreviewForget] = useState<ReviewOut | null>(null)

  const decks = useMemo(() => {
    const map = new Map<string, { total: number; due: number }>()
    for (const card of state.cards) {
      const entry = map.get(card.deck) ?? { total: 0, due: 0 }
      entry.total += 1
      if (!card.paused && card.due && card.due <= today_) entry.due += 1
      map.set(card.deck, entry)
    }
    return [...map.entries()].sort((a, b) => a[0].localeCompare(b[0]))
  }, [state.cards, today_])

  const refreshQueue = async () => {
    try {
      const out = await callPlugin<QueueOut>(manifest.id, 'queue', {
        cards: state.cards.map((card) => ({
          id: card.id,
          deck: card.deck,
          stage: card.stage,
          due: card.due,
          paused: card.paused,
        })),
        today: today_,
        limit: state.sessionLimit,
        newLimit: state.newLimit,
      })
      setQueue(out)
      setQueueIndex(0)
      setRevealed(false)
      setSessionDone(false)
      setPreviewRemember(null)
      setPreviewForget(null)
      // 预取第一张卡的评分预览（下次日期按钮提示）
      const firstId = [...out.dueIds, ...out.newIds][0]
      const first = state.cards.find((card) => card.id === firstId)
      if (first) {
        void loadPreview(first)
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const sessionCards: Card[] = useMemo(() => {
    if (!queue) return []
    const byId = new Map(state.cards.map((card) => [card.id, card]))
    return [...queue.dueIds, ...queue.newIds].map((id) => byId.get(id)).filter((card): card is Card => !!card)
  }, [queue, state.cards])

  const current = sessionCards[queueIndex] ?? null

  // 评分前预览两个选择对应的下次日期（后端计算档位与日历）
  const loadPreview = async (card: Card) => {
    try {
      const [remember, forget] = await Promise.all([
        callPlugin<ReviewOut>(manifest.id, 'review', { stage: card.stage, today: today_, remembered: true }),
        callPlugin<ReviewOut>(manifest.id, 'review', { stage: card.stage, today: today_, remembered: false }),
      ])
      setPreviewRemember(remember)
      setPreviewForget(forget)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const grade = async (remembered: boolean) => {
    if (!current) return
    // 每张卡每轮只评一次：评分后立即移出队列
    const card = current
    try {
      const out = await callPlugin<ReviewOut>(manifest.id, 'review', {
        stage: card.stage,
        today: today_,
        remembered,
      })
      const entry: ReviewLogEntry = {
        date: today_,
        remembered,
        prevStage: card.stage,
        prevDue: card.due,
        nextDue: out.due,
      }
      setState({
        ...state,
        cards: state.cards.map((item) =>
          item.id === card.id
            ? { ...item, stage: out.stage, due: out.due, history: [...item.history, entry] }
            : item,
        ),
        lastUndo: { cardId: card.id, entry },
      })
      const nextIndex = queueIndex + 1
      setRevealed(false)
      setPreviewRemember(null)
      setPreviewForget(null)
      if (nextIndex >= sessionCards.length) {
        setSessionDone(true)
      } else {
        setQueueIndex(nextIndex)
        const next = sessionCards[nextIndex]
        if (next) void loadPreview(next)
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const skip = () => {
    // 跳过不计分：移出本轮队列（未展示卡不算已复习）
    const nextIndex = queueIndex + 1
    setRevealed(false)
    setPreviewRemember(null)
    setPreviewForget(null)
    if (nextIndex >= sessionCards.length) {
      setSessionDone(true)
    } else {
      setQueueIndex(nextIndex)
      const next = sessionCards[nextIndex]
      if (next) void loadPreview(next)
    }
  }

  const undo = () => {
    const { lastUndo } = state
    if (!lastUndo) return
    setState({
      ...state,
      cards: state.cards.map((card) =>
        card.id === lastUndo.cardId
          ? { ...card, stage: lastUndo.entry.prevStage, due: lastUndo.entry.prevDue, history: card.history.slice(0, -1) }
          : card,
      ),
      lastUndo: null,
    })
    toast.success(t('common:saved'))
  }

  // ---- 管理 ----
  const [newQuestion, setNewQuestion] = useState('')
  const [newAnswer, setNewAnswer] = useState('')
  const [newDeck, setNewDeck] = useState('')
  const [newTags, setNewTags] = useState('')

  const addCard = async () => {
    const question = newQuestion.trim()
    if (!question) return
    try {
      const { id } = await callPlugin<{ id: string }>(manifest.id, 'new_id', {})
      setState({
        ...state,
        cards: [
          ...state.cards,
          {
            id,
            deck: newDeck.trim() || t('deck.default'),
            question,
            answer: newAnswer.trim(),
            tags: newTags
              .split(';')
              .map((tag) => tag.trim())
              .filter(Boolean),
            paused: false,
            stage: 0,
            due: '',
            history: [],
            createdAt: today_,
          },
        ],
      })
      setNewQuestion('')
      setNewAnswer('')
      setNewTags('')
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const patchCard = (id: string, fields: Partial<Card>) => {
    // 编辑问答/标签不重置学习进度（stage/due/history 不动）
    setState({ ...state, cards: state.cards.map((card) => (card.id === id ? { ...card, ...fields } : card)) })
  }

  const removeCard = (id: string) => {
    setState({ ...state, cards: state.cards.filter((card) => card.id !== id) })
  }

  // ---- 导入 ----
  const [csvText, setCsvText] = useState('')
  const [csvDefaultDeck, setCsvDefaultDeck] = useState('')
  const [csvErrors, setCsvErrors] = useState<{ line: number; rule: string }[]>([])
  const [importedCount, setImportedCount] = useState(0)

  const runImport = async () => {
    try {
      const out = await callPlugin<ImportOut>(manifest.id, 'import_csv', {
        csv: csvText,
        defaultDeck: csvDefaultDeck,
      })
      setCsvErrors(out.errors)
      setImportedCount(out.cards.length)
      if (out.cards.length > 0) {
        const created: Card[] = []
        for (const item of out.cards) {
          const { id } = await callPlugin<{ id: string }>(manifest.id, 'new_id', {})
          created.push({
            id,
            deck: item.deck || t('deck.default'),
            question: item.question,
            answer: item.answer,
            tags: item.tags,
            paused: false,
            stage: 0,
            due: '',
            history: [],
            createdAt: today_,
          })
        }
        setState({ ...state, cards: [...state.cards, ...created] })
        toast.success(t('import.done', { count: out.cards.length }))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  // ---- 备份 ----
  const [restoreText, setRestoreText] = useState('')
  const [restorePreview, setRestorePreview] = useState<PreviewOut | null>(null)

  const exportBackup = async () => {
    try {
      const out = await callPlugin<ExportOut>(manifest.id, 'backup', {
        cards: state.cards.map(toBackupCard),
      })
      const path = await host.dialog.saveFile('flashcards-backup.json', [{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await host.fs.writeText(path, out.json)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const runPreview = async () => {
    try {
      const out = await callPlugin<PreviewOut>(manifest.id, 'restore_preview', {
        json: restoreText,
        current: state.cards.map(toBackupCard),
      })
      setRestorePreview(out)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const applyRestore = async (mode: 'overwrite' | 'skip' | 'replace') => {
    try {
      const out = await callPlugin<ApplyOut>(manifest.id, 'restore_apply', {
        json: restoreText,
        current: state.cards.map(toBackupCard),
        mode,
      })
      setState({ ...state, cards: out.cards.map(toCard), lastUndo: null })
      setRestorePreview(null)
      setRestoreText('')
      toast.success(t('backup.applied', { count: out.cards.length }))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const deckView = view.kind === 'deck' ? state.cards.filter((card) => card.deck === view.name) : []
  const reviewedCount = queue ? queueIndex : 0
  const sessionTotal = sessionCards.length

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(340px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        <Panel
          icon={<CalendarClock />}
          title={t('session.title')}
          bodyClassName="space-y-2 p-3"
          actions={
            <Button size="sm" variant="ghost" onClick={() => void refreshQueue()} disabled={state.cards.length === 0}>
              {t('session.loadQueue')}
            </Button>
          }
        >
          <div className="flex items-center gap-2">
            <Input
              value={state.sessionLimit}
              onChange={(event) => setState({ ...state, sessionLimit: Number(event.target.value.replace(/[^\d]/g, '')) || 0 })}
              size="sm"
              aria-label={t('session.limit')}
              wrapperClassName="w-20"
              inputMode="numeric"
            />
            <span className="text-xs text-fg-muted">{t('session.limitUnit')}</span>
            <Input
              value={state.newLimit}
              onChange={(event) => setState({ ...state, newLimit: Number(event.target.value.replace(/[^\d]/g, '')) || 0 })}
              size="sm"
              aria-label={t('session.newLimit')}
              wrapperClassName="w-20"
              inputMode="numeric"
            />
            <span className="text-xs text-fg-muted">{t('session.newLimitUnit')}</span>
          </div>
          {queue && (
            <div className="space-y-1 text-xs text-fg-muted">
              <p>{t('session.counts', { due: queue.counts.due, fresh: queue.counts.new, paused: queue.counts.paused })}</p>
              {queue.nextDue && <p>{t('session.nextDue', { date: queue.nextDue })}</p>}
              {sessionTotal > 0 && <Progress value={(reviewedCount / sessionTotal) * 100} />}
            </div>
          )}
          {state.lastUndo && (
            <Tooltip content={t('session.undoTip')}>
              <Button size="sm" variant="ghost" onClick={undo}>
                <RotateCcw />
                {t('session.undo')}
              </Button>
            </Tooltip>
          )}
        </Panel>

        <Panel icon={<Layers />} title={t('deck.title')} bodyClassName="space-y-1 p-3">
          {decks.length === 0 && <p className="text-xs text-fg-subtle">{t('deck.empty')}</p>}
          {decks.map(([name, info]) => (
            <button
              key={name}
              type="button"
              onClick={() => setView({ kind: 'deck', name })}
              className="flex w-full items-center justify-between rounded-lg border border-border-subtle px-2 py-1.5 text-left hover:bg-bg-subtle"
            >
              <span className="truncate text-sm">{name || t('deck.default')}</span>
              <span className="flex items-center gap-1">
                {info.due > 0 && (
                  <Badge variant="warning">{t('deck.dueCount', { count: info.due })}</Badge>
                )}
                <Badge variant="neutral">{t('deck.totalCount', { count: info.total })}</Badge>
              </span>
            </button>
          ))}
          <div className="flex gap-2 pt-1">
            <Button size="sm" variant="ghost" onClick={() => setView({ kind: 'review' })}>
              <BookOpen />
              {t('nav.review')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setView({ kind: 'import' })}>
              <Upload />
              {t('nav.import')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setView({ kind: 'backup' })}>
              <Download />
              {t('nav.backup')}
            </Button>
          </div>
        </Panel>

        <Panel icon={<Plus />} title={t('add.title')} bodyClassName="space-y-2 p-3">
          <Input
            value={newQuestion}
            onChange={(event) => setNewQuestion(event.target.value)}
            placeholder={t('add.question')}
            size="sm"
            aria-label={t('add.question')}
          />
          <Input
            value={newAnswer}
            onChange={(event) => setNewAnswer(event.target.value)}
            placeholder={t('add.answer')}
            size="sm"
            aria-label={t('add.answer')}
          />
          <div className="flex items-center gap-2">
            <Input
              value={newDeck}
              onChange={(event) => setNewDeck(event.target.value)}
              placeholder={t('add.deck')}
              size="sm"
              aria-label={t('add.deck')}
              wrapperClassName="flex-1"
            />
            <Input
              value={newTags}
              onChange={(event) => setNewTags(event.target.value)}
              placeholder={t('add.tags')}
              size="sm"
              aria-label={t('add.tags')}
              wrapperClassName="flex-1"
            />
            <Button size="sm" variant="ghost" onClick={() => void addCard()} disabled={!newQuestion.trim()}>
              <Plus />
              {t('add.submit')}
            </Button>
          </div>
        </Panel>
      </div>

      <div className="min-h-0">
        {view.kind === 'review' && (
          <Panel icon={<BookOpen />} title={t('review.title')} bodyClassName="flex min-h-0 flex-1 flex-col gap-3 p-3">
            {!queue && <Empty title={t('review.idle')} />}
            {queue && sessionDone && (
              <Empty title={t('review.done', { count: sessionCards.length })} />
            )}
            {queue && !sessionDone && current && (
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <div className="flex items-center gap-2">
                  <Badge variant="neutral">{current.deck || t('deck.default')}</Badge>
                  {current.tags.map((tag) => (
                    <Badge key={tag} variant="neutral">
                      {tag}
                    </Badge>
                  ))}
                  {queue.newIds.includes(current.id) && <Badge variant="warning">{t('review.isNew')}</Badge>}
                  <span className="ml-auto text-xs text-fg-subtle">
                    {t('review.position', { current: queueIndex + 1, total: sessionTotal })}
                  </span>
                </div>
                <div className="flex-1 space-y-3 overflow-y-auto rounded-lg border border-border-subtle p-3">
                  <p className="text-sm whitespace-pre-wrap">{current.question}</p>
                  {revealed ? (
                    <p className="border-t border-border-subtle pt-3 text-sm whitespace-pre-wrap text-fg-muted">{current.answer}</p>
                  ) : (
                    <Button variant="ghost" onClick={() => setRevealed(true)}>
                      <Eye />
                      {t('review.reveal')}
                    </Button>
                  )}
                </div>
                {revealed && (
                  <div className="flex flex-wrap items-center gap-2">
                    <Button variant="ghost" onClick={() => void grade(false)}>
                      <X />
                      {t('review.forgot')}
                      {previewForget && <span className="ml-1 text-xs">{t('review.nextDate', { date: previewForget.due })}</span>}
                    </Button>
                    <Button onClick={() => void grade(true)}>
                      <Check />
                      {t('review.remembered')}
                      {previewRemember && <span className="ml-1 text-xs">{t('review.nextDate', { date: previewRemember.due })}</span>}
                    </Button>
                    <Button variant="ghost" onClick={skip}>
                      {t('review.skip')}
                    </Button>
                  </div>
                )}
              </div>
            )}
          </Panel>
        )}

        {view.kind === 'deck' && (
          <Panel icon={<Layers />} title={view.name || t('deck.default')} bodyClassName="flex min-h-0 flex-1 flex-col gap-2 p-3">
            {deckView.length === 0 && <Empty title={t('deck.emptyDeck')} />}
            <div className="min-h-0 flex-1 space-y-2 overflow-y-auto">
              {deckView.map((card) => (
                <CardRow key={card.id} card={card} onPatch={patchCard} onRemove={removeCard} />
              ))}
            </div>
          </Panel>
        )}

        {view.kind === 'import' && (
          <Panel icon={<Upload />} title={t('import.title')} bodyClassName="flex min-h-0 flex-1 flex-col gap-2 p-3">
            <p className="text-xs text-fg-subtle">{t('import.format')}</p>
            <textarea
              value={csvText}
              onChange={(event) => setCsvText(event.target.value)}
              placeholder={'question,answer,deck,tags\n"Bonjour","你好","法语","问候;入门"'}
              rows={8}
              aria-label={t('import.csvLabel')}
              className="w-full flex-1 resize-none rounded-md border border-border-subtle bg-transparent p-2 font-mono text-xs outline-none focus:border-primary-500"
            />
            <div className="flex items-center gap-2">
              <Input
                value={csvDefaultDeck}
                onChange={(event) => setCsvDefaultDeck(event.target.value)}
                placeholder={t('import.defaultDeck')}
                size="sm"
                aria-label={t('import.defaultDeck')}
                wrapperClassName="w-40"
              />
              <Button size="sm" onClick={() => void runImport()} disabled={!csvText.trim()}>
                {t('import.run')}
              </Button>
              {importedCount > 0 && <Badge variant="neutral">{t('import.done', { count: importedCount })}</Badge>}
            </div>
            {csvErrors.length > 0 && (
              <div className="space-y-0.5 text-xs text-fg-danger">
                {csvErrors.map((error) => (
                  <p key={`${error.line}-${error.rule}`}>
                    {t('import.lineError', { line: error.line })} · {t(`errors.${error.rule}`)}
                  </p>
                ))}
              </div>
            )}
          </Panel>
        )}

        {view.kind === 'backup' && (
          <Panel icon={<Download />} title={t('backup.title')} bodyClassName="flex min-h-0 flex-1 flex-col gap-2 p-3">
            <div className="flex items-center gap-2">
              <Button size="sm" variant="ghost" onClick={() => void exportBackup()} disabled={state.cards.length === 0}>
                <Download />
                {t('backup.export')}
              </Button>
            </div>
            <p className="text-xs text-fg-subtle">{t('backup.pasteHint')}</p>
            <textarea
              value={restoreText}
              onChange={(event) => {
                setRestoreText(event.target.value)
                setRestorePreview(null)
              }}
              rows={6}
              aria-label={t('backup.pasteLabel')}
              className="w-full flex-1 resize-none rounded-md border border-border-subtle bg-transparent p-2 font-mono text-xs outline-none focus:border-primary-500"
            />
            <div className="flex items-center gap-2">
              <Button size="sm" variant="ghost" onClick={() => void runPreview()} disabled={!restoreText.trim()}>
                {t('backup.preview')}
              </Button>
            </div>
            {restorePreview && (
              <div className="space-y-2 rounded-lg border border-border-subtle p-2 text-xs">
                <p>
                  {t('backup.incoming', { count: restorePreview.incoming })} ·{' '}
                  {t('backup.fresh', { count: restorePreview.fresh })} ·{' '}
                  {t('backup.conflicts', { count: restorePreview.conflicts.length })}
                </p>
                {restorePreview.conflicts.slice(0, 5).map((conflict) => (
                  <p key={conflict.id} className="truncate font-mono text-fg-warning">
                    {conflict.question}
                  </p>
                ))}
                <div className="flex flex-wrap gap-2">
                  <Button size="sm" variant="ghost" onClick={() => void applyRestore('overwrite')} disabled={restorePreview.conflicts.length === 0}>
                    {t('backup.overwrite')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => void applyRestore('skip')} disabled={restorePreview.conflicts.length === 0}>
                    {t('backup.skipConflicts')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => void applyRestore('replace')}>
                    {t('backup.replace')}
                  </Button>
                </div>
              </div>
            )}
          </Panel>
        )}
      </div>
    </div>
  )
}

function CardRow({
  card,
  onPatch,
  onRemove,
}: {
  card: Card
  onPatch: (id: string, fields: Partial<Card>) => void
  onRemove: (id: string) => void
}) {
  const { t } = usePlugin()
  const [editing, setEditing] = useState(false)
  const [expanded, setExpanded] = useState(false)
  const [question, setQuestion] = useState(card.question)
  const [answer, setAnswer] = useState(card.answer)

  return (
    <div className="space-y-1 rounded-lg border border-border-subtle p-2">
      <div className="flex items-center justify-between gap-2">
        <button type="button" className="min-w-0 flex-1 truncate text-left text-sm" onClick={() => setExpanded(!expanded)}>
          {card.question}
        </button>
        <div className="flex shrink-0 items-center gap-1">
          {card.stage === 0 && !card.due && <Badge variant="warning">{t('review.isNew')}</Badge>}
          {card.due && <Badge variant="neutral">{card.due}</Badge>}
          <Tooltip content={card.paused ? t('card.resume') : t('card.pause')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={card.paused ? t('card.resume') : t('card.pause')}
              onClick={() => onPatch(card.id, { paused: !card.paused })}
            >
              {card.paused ? <EyeOff /> : <Eye />}
            </Button>
          </Tooltip>
          <Tooltip content={t('card.edit')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('card.edit')} onClick={() => setEditing(!editing)}>
              <Pencil />
            </Button>
          </Tooltip>
          <Tooltip content={t('card.remove')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('card.remove')} onClick={() => onRemove(card.id)}>
              <Trash2 />
            </Button>
          </Tooltip>
        </div>
      </div>
      {editing && (
        <div className="space-y-1">
          <Input value={question} onChange={(event) => setQuestion(event.target.value)} size="sm" aria-label={t('add.question')} />
          <Input value={answer} onChange={(event) => setAnswer(event.target.value)} size="sm" aria-label={t('add.answer')} />
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                // 编辑问答不重置学习进度
                onPatch(card.id, { question: question.trim() || card.question, answer: answer.trim() })
                setEditing(false)
              }}
            >
              {t('common:saved')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
              {t('card.cancelEdit')}
            </Button>
          </div>
        </div>
      )}
      {expanded && (
        <div className="space-y-0.5 text-xs text-fg-subtle">
          <p className="whitespace-pre-wrap">{card.answer || '—'}</p>
          <p>
            {t('card.stageLabel', { stage: card.stage })} · {card.due || t('card.noDue')}
          </p>
          {card.history.slice(-5).map((entry, index) => (
            <p key={index} className="font-mono">
              {t('card.historyLine', {
                date: entry.date,
                verdict: entry.remembered ? t('review.remembered') : t('review.forgot'),
                from: entry.prevStage,
                to: entry.remembered ? entry.prevStage + 1 : 0,
                next: entry.nextDue,
              })}
            </p>
          ))}
        </div>
      )}
    </div>
  )
}
