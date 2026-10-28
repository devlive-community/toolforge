import { useCallback, useEffect, useRef, useState } from 'react'
import { Badge, Button, Drawer, Empty, Input, Modal, Pagination, SegmentedControl, Select, Spinner, Tabs, Tooltip, cn, toast } from '@toolforge/ui'
import { callPlugin, host, loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BookOpen, CalendarClock, Camera, CheckCircle2, Clapperboard, Download, FileJson, Pencil, Play, Plus, Search, Trash2, Undo2, Upload } from 'lucide-react'
import { CompleteForm, ConfirmForm, SessionForm, WorkForm, stars, type Actions } from './Forms'
import { localDate, newOpId } from './today'
import type { Journal as JournalData, Kind, Op, Outcome, Restored, Review, SessionView, Status, View, WorkView } from './types'

const JOURNAL_KEY = 'journal'
const coverKey = (id: string) => `cover:${id}`
const PAGE = 20
const STATUSES = ['want', 'doing', 'done', 'paused'] as const

type Dialog =
  | { kind: 'work'; work: WorkView | null }
  | { kind: 'session'; session: SessionView }
  | { kind: 'complete'; session: SessionView }
  | { kind: 'uncomplete'; session: SessionView }
  | { kind: 'deleteSession'; session: SessionView }
  | { kind: 'deleteWork'; work: WorkView }
  | { kind: 'undo' }
  | null

/** 没有封面时的原创文字占位卡：取名称首字，按类型配色 */
function Placeholder({ work, className }: { work: WorkView; className?: string }) {
  const first = Array.from(work.title.trim())[0] ?? '?'
  return (
    <span
      aria-hidden
      className={cn(
        'flex shrink-0 items-center justify-center rounded-control text-[18px] font-semibold',
        work.kind === 'book' ? 'bg-primary-soft text-primary' : 'bg-info-soft text-info',
        className,
      )}
    >
      {first}
    </span>
  )
}

export function Journal() {
  const { t, manifest, errorMessage } = usePlugin()
  const pluginId = manifest.id
  const [journal, setJournal] = useState<JournalData | null>(null)
  const [view, setView] = useState<View | null>(null)
  const [unsaved, setUnsaved] = useState<JournalData | null>(null)
  const [busy, setBusy] = useState(false)
  const [tab, setTab] = useState<'works' | 'review' | 'pending'>('works')
  const [search, setSearch] = useState('')
  const [kind, setKind] = useState<'' | Kind>('')
  const [status, setStatus] = useState<'' | Status>('')
  const [tag, setTag] = useState('')
  const [minRating, setMinRating] = useState(0)
  const [page, setPage] = useState(1)
  const [year, setYear] = useState(() => new Date().getFullYear())
  const [review, setReview] = useState<Review | null>(null)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [detailId, setDetailId] = useState<string | null>(null)
  const [detail, setDetail] = useState<{ work: WorkView; sessions: SessionView[] } | null>(null)
  const [covers, setCovers] = useState<Record<string, string>>({})
  const [restore, setRestore] = useState<{ text: string; mode: 'merge' | 'replace'; preview: Restored | null; error: string | null } | null>(null)
  const journalRef = useRef<JournalData | null>(null)
  const today = localDate()

  const show = (next: JournalData) => {
    journalRef.current = next
    setJournal(next)
  }

  useEffect(() => {
    let alive = true
    loadPluginState<JournalData>(pluginId, JOURNAL_KEY)
      .then(async (saved) => saved ?? (await callPlugin<JournalData>(pluginId, 'empty', {})))
      .then((value) => alive && show(value))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [pluginId, errorMessage])

  useEffect(() => {
    if (!journal) return
    let alive = true
    callPlugin<View>(pluginId, 'view', {
      journal,
      search,
      kind: kind || null,
      status: status || null,
      tag: tag || null,
      minRating: minRating || null,
      offset: (page - 1) * PAGE,
      limit: PAGE,
    })
      .then((v) => alive && setView(v))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [pluginId, journal, search, kind, status, tag, minRating, page, errorMessage])

  useEffect(() => {
    if (!journal || tab !== 'review') return
    let alive = true
    callPlugin<Review>(pluginId, 'review', { journal, year })
      .then((r) => alive && setReview(r))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [pluginId, journal, tab, year, errorMessage])

  useEffect(() => {
    if (!journal || !detailId) return
    let alive = true
    callPlugin<{ work: WorkView; sessions: SessionView[] }>(pluginId, 'history', { journal, workId: detailId })
      .then((d) => alive && setDetail(d))
      .catch(() => alive && setDetail(null))
    return () => {
      alive = false
    }
  }, [pluginId, journal, detailId])

  // 封面按需读取
  const wanted = [...(view?.works ?? []), ...(detail ? [detail.work] : [])].filter((w) => w.hasCover && covers[w.id] === undefined).map((w) => w.id)
  const wantedKey = wanted.join(',')
  useEffect(() => {
    if (!wantedKey) return
    let alive = true
    Promise.all(wantedKey.split(',').map(async (id) => [id, (await loadPluginState<string>(pluginId, coverKey(id))) ?? ''] as const))
      .then((entries) => alive && setCovers((c) => ({ ...c, ...Object.fromEntries(entries) })))
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [pluginId, wantedKey])

  /** 保存手账；成功才更新界面，失败保留结果并锁住其他修改 */
  const persist = useCallback(
    async (next: JournalData) => {
      try {
        await savePluginState(pluginId, JOURNAL_KEY, next)
      } catch (error) {
        setUnsaved(next)
        toast.error(t('save.failed', { reason: errorMessage(error) }))
        return false
      }
      setUnsaved(null)
      show(next)
      return true
    },
    [pluginId, t, errorMessage],
  )

  const actions: Actions = {
    busy: busy || unsaved !== null,
    commit: async (op) => {
      const current = journalRef.current
      if (!current || unsaved) return false
      setBusy(true)
      try {
        // 校验失败直接抛给表单显示
        const out = await callPlugin<Outcome>(pluginId, 'apply', { journal: current, op, opId: newOpId(), revision: current.revision, today })
        const saved = await persist(out.journal)
        if (saved && !out.duplicate) toast.success(t(`done.${op.kind}`))
        return saved
      } finally {
        setBusy(false)
      }
    },
  }

  const quick = async (op: Op) => {
    try {
      await actions.commit(op)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const retrySave = async () => {
    if (!unsaved) return
    setBusy(true)
    if (await persist(unsaved)) toast.success(t('save.retried'))
    setBusy(false)
  }

  const chooseCover = async (work: WorkView) => {
    try {
      const path = await host.dialog.openFile([{ name: t('cover.images'), extensions: ['jpg', 'jpeg', 'png', 'webp'] }])
      if (!path) return
      const { data } = await callPlugin<{ data: string }>(pluginId, 'cover', { path })
      await savePluginState(pluginId, coverKey(work.id), data)
      setCovers((c) => ({ ...c, [work.id]: data }))
      await quick({ kind: 'setCover', workId: work.id, hasCover: true })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const removeCover = async (work: WorkView) => {
    try {
      if (await actions.commit({ kind: 'setCover', workId: work.id, hasCover: false })) {
        await savePluginState(pluginId, coverKey(work.id), null).catch(() => undefined)
        setCovers((c) => ({ ...c, [work.id]: '' }))
      }
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const exportList = async () => {
    const current = journalRef.current
    if (!current) return
    try {
      const headers = ['kind', 'title', 'creator', 'year', 'genre', 'tags', 'status', 'total', 'start', 'finish', 'progress', 'completed', 'rating', 'review'].map((k) => t(`csv.${k}`))
      const labels = { book: t('kind.book'), film: t('kind.film'), want: t('status.want.book'), doing: t('status.doing.book'), done: t('status.done.book'), paused: t('status.paused.book') }
      const { content } = await callPlugin<{ content: string }>(pluginId, 'exportList', { journal: current, headers, labels })
      const path = await host.dialog.saveFile(`reading-journal-${today}.csv`, [{ name: 'CSV', extensions: ['csv'] }])
      if (!path) return
      await host.fs.writeText(path, content)
      toast.success(t('export.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const exportBackup = async () => {
    const current = journalRef.current
    if (!current) return
    try {
      const ids = current.works.filter((w) => w.hasCover).map((w) => w.id)
      const loaded = await Promise.all(ids.map(async (id) => [id, (await loadPluginState<string>(pluginId, coverKey(id))) ?? ''] as const))
      const { content } = await callPlugin<{ content: string }>(pluginId, 'exportBackup', { journal: current, covers: Object.fromEntries(loaded.filter(([, d]) => d)), today })
      const path = await host.dialog.saveFile(`reading-journal-backup-${today}.json`, [{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await host.fs.writeText(path, content)
      toast.success(t('export.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const previewRestore = async (text: string, mode: 'merge' | 'replace') => {
    const current = journalRef.current
    if (!current) return
    setRestore({ text, mode, preview: null, error: null })
    try {
      const preview = await callPlugin<Restored>(pluginId, 'restore', { journal: current, text, mode, revision: current.revision, preview: true })
      setRestore({ text, mode, preview, error: null })
    } catch (error) {
      setRestore({ text, mode, preview: null, error: errorMessage(error) })
    }
  }

  const openRestore = async () => {
    try {
      const path = await host.dialog.openFile([{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await previewRestore(await host.fs.readText(path), 'merge')
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 先写入新封面，再保存手账，最后删掉被替换作品的封面 */
  const confirmRestore = async () => {
    const current = journalRef.current
    if (!restore || !current || unsaved) return
    setBusy(true)
    try {
      const out = await callPlugin<Restored>(pluginId, 'restore', { journal: current, text: restore.text, mode: restore.mode, revision: current.revision, preview: false })
      if (!out.journal) return
      for (const [id, data] of Object.entries(out.covers)) await savePluginState(pluginId, coverKey(id), data)
      if (!(await persist(out.journal))) return
      const kept = new Set(Object.keys(out.covers))
      for (const id of out.removeCovers.filter((id) => !kept.has(id))) await savePluginState(pluginId, coverKey(id), null).catch(() => undefined)
      setCovers({})
      setRestore(null)
      toast.success(t('restore.done'))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  if (!journal || !view) {
    return (
      <div className="flex h-full items-center justify-center p-3">
        <Spinner />
      </div>
    )
  }

  const locked = busy || unsaved !== null
  const unitOf = (k: Kind) => (k === 'book' ? t('unit.pages') : t('unit.minutes'))
  /** 进度：总量已知显示「已读 / 总量」，未知只显示当前进度，不伪造百分比 */
  const progressText = (progress: number | null, total: number | null, k: Kind) =>
    progress === null ? '' : total ? t('progress.known', { progress, total, unit: unitOf(k) }) : t('progress.unknown', { progress, unit: unitOf(k) })
  const statusLabel = (s: Status, k: Kind) => t(`status.${s}.${k}`)
  const cover = (work: WorkView, size: string) =>
    work.hasCover && covers[work.id] ? <img src={covers[work.id]} alt="" className={cn('shrink-0 rounded-control object-cover', size)} /> : <Placeholder work={work} className={size} />

  const workCard = (work: WorkView) => (
    <div key={work.id} className="flex gap-3 rounded-card border border-border bg-surface p-3">
      {cover(work, 'h-16 w-12')}
      <div className="min-w-0 flex-1 space-y-1">
        <button type="button" onClick={() => setDetailId(work.id)} className="block max-w-full truncate text-left text-[13px] font-semibold text-fg hover:underline">
          {work.title}
        </button>
        <p className="truncate text-[11px] text-fg-muted">{[work.creator, work.year, work.genre].filter(Boolean).join(' · ') || '—'}</p>
        <div className="flex flex-wrap gap-1">
          <Badge variant={work.status === 'done' ? 'success' : work.status === 'doing' ? 'info' : 'neutral'}>{statusLabel(work.status, work.kind)}</Badge>
          {work.tags.map((tg) => (
            <Badge key={tg}>{tg}</Badge>
          ))}
        </div>
        <p className="text-[12px] text-fg-muted">
          {[progressText(work.progress, work.total, work.kind), work.completions > 0 ? t('work.completions', { count: work.completions }) : '', stars(work.bestRating)].filter(Boolean).join(' · ')}
        </p>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1">
        <Button size="sm" variant="secondary" disabled={locked} onClick={() => void quick({ kind: 'start', workId: work.id, start: today })}>
          <Play />
          {work.sessions > 0 ? t('session.again', { verb: t(`verb.${work.kind}`) }) : t('session.begin', { verb: t(`verb.${work.kind}`) })}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setDetailId(work.id)}>
          {t('work.details')}
        </Button>
      </div>
    </div>
  )

  const sessionRow = (s: SessionView, showTitle: boolean) => (
    <div key={s.id} className="space-y-1 rounded-card border border-border bg-surface p-3 text-[12px]">
      <div className="flex flex-wrap items-center gap-2">
        {showTitle && <span className="font-semibold text-fg">{s.title}</span>}
        {s.completed ? <Badge variant="success">{s.finish ? t('session.finishedOn', { date: s.finish }) : t('session.finishMissing')}</Badge> : <Badge variant="info">{t('session.inProgress')}</Badge>}
        {s.rating && <span className="text-warning">{stars(s.rating)}</span>}
        <div className="ml-auto flex items-center gap-1">
          <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'session', session: s })}>
            <Pencil />
            {s.completed && !s.finish ? t('session.fillFinish') : t('session.edit')}
          </Button>
          {s.completed ? (
            <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'uncomplete', session: s })}>
              {t('complete.undo')}
            </Button>
          ) : (
            <Button size="sm" variant="secondary" disabled={locked} onClick={() => setDialog({ kind: 'complete', session: s })}>
              <CheckCircle2 />
              {t('complete.action')}
            </Button>
          )}
          <Tooltip content={t('session.delete')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('session.delete')} disabled={locked} onClick={() => setDialog({ kind: 'deleteSession', session: s })}>
              <Trash2 />
            </Button>
          </Tooltip>
        </div>
      </div>
      <p className="text-fg-muted">
        {[s.start ? t('session.startedOn', { date: s.start }) : t('session.noStart'), progressText(s.progress, s.total, s.kind)].filter(Boolean).join(' · ')}
      </p>
      {s.review && <p className="whitespace-pre-wrap text-fg">{s.review}</p>}
    </div>
  )

  const yearOptions = [...new Set([new Date().getFullYear(), ...view.reviewYears])].sort((a, b) => b - a).map((y) => ({ value: y, label: String(y) }))
  const maxMonth = Math.max(1, ...(review?.months ?? [0]))

  return (
    <div className="space-y-3 p-3">
      {unsaved && (
        <div role="alert" className="flex flex-wrap items-center gap-2 rounded-card border border-danger bg-danger-soft p-3 text-[13px] text-fg">
          <span className="flex-1">{t('save.pending')}</span>
          <Button size="sm" variant="danger" loading={busy} onClick={() => void retrySave()}>
            {t('save.retry')}
          </Button>
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {STATUSES.map((s) => (
          <Badge key={s} variant={s === 'done' ? 'success' : s === 'doing' ? 'info' : 'neutral'}>
            {t(`counts.${s}`, { count: view.counts[s] })}
          </Badge>
        ))}
        <div className="ml-auto flex items-center gap-1">
          {view.undo && (
            <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'undo' })}>
              <Undo2 />
              {t(`undo.${view.undo}`)}
            </Button>
          )}
          <Tooltip content={t('export.list')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('export.list')} onClick={() => void exportList()}>
              <Download />
            </Button>
          </Tooltip>
          <Tooltip content={t('export.backup')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('export.backup')} onClick={() => void exportBackup()}>
              <FileJson />
            </Button>
          </Tooltip>
          <Tooltip content={t('restore.action')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('restore.action')} disabled={locked} onClick={() => void openRestore()}>
              <Upload />
            </Button>
          </Tooltip>
          <Button size="sm" variant="primary" disabled={locked} onClick={() => setDialog({ kind: 'work', work: null })}>
            <Plus />
            {t('work.new')}
          </Button>
        </div>
      </div>

      <Tabs
        value={tab}
        onValueChange={setTab}
        items={[
          { value: 'works', label: t('tabs.works'), icon: <BookOpen /> },
          { value: 'review', label: t('tabs.review'), icon: <Clapperboard /> },
          { value: 'pending', label: t('tabs.pending', { count: view.pending.length }), icon: <CalendarClock /> },
        ]}
      />

      {tab === 'works' && (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <Input size="sm" value={search} onChange={(e) => { setSearch(e.target.value); setPage(1) }} placeholder={t('filter.search')} aria-label={t('filter.search')} leading={<Search />} wrapperClassName="min-w-48 flex-1" />
            <Select<'' | Kind> size="sm" className="w-28" value={kind} onValueChange={(v) => { setKind(v); setPage(1) }} options={[{ value: '', label: t('filter.allKinds') }, { value: 'book', label: t('kind.book') }, { value: 'film', label: t('kind.film') }]} />
            <Select<'' | Status> size="sm" className="w-32" value={status} onValueChange={(v) => { setStatus(v); setPage(1) }} options={[{ value: '', label: t('filter.allStatuses') }, ...STATUSES.map((s) => ({ value: s, label: t(`filter.status.${s}`) }))]} />
            <Select<string> size="sm" className="w-32" value={tag} onValueChange={(v) => { setTag(v); setPage(1) }} options={[{ value: '', label: t('filter.allTags') }, ...view.tags.map((tg) => ({ value: tg, label: tg }))]} />
            <Select<number> size="sm" className="w-32" value={minRating} onValueChange={(v) => { setMinRating(v); setPage(1) }} options={[{ value: 0, label: t('filter.anyRating') }, ...[10, 8, 6, 4, 2].map((r) => ({ value: r, label: t('filter.atLeast', { stars: stars(r) }) }))]} />
          </div>
          {view.works.length === 0 ? (
            <Empty className="py-12" icon={<BookOpen />} title={journal.works.length === 0 ? t('empty.none') : t('empty.filtered')} />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(340px,1fr))] gap-2">{view.works.map(workCard)}</div>
          )}
          {view.total > PAGE && (
            <Pagination page={page} pageSize={PAGE} total={view.total} onPageChange={setPage} prevLabel={t('page.prev')} nextLabel={t('page.next')} pageLabel={(p) => t('page.number', { page: p })} totalLabel={t('page.total', { count: view.total })} />
          )}
        </div>
      )}

      {tab === 'review' && (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Select<number> size="sm" className="w-28" value={year} onValueChange={setYear} options={yearOptions} />
            <p className="text-[12px] text-fg-muted">{t('review.hint')}</p>
          </div>
          {review && (
            <>
              <div className="grid grid-cols-[repeat(auto-fill,minmax(130px,1fr))] gap-2">
                {[
                  [t('review.works'), review.works],
                  [t('review.completions'), review.completions],
                  [t('review.books'), review.books],
                  [t('review.films'), review.films],
                  [t('review.pages'), review.pages],
                  [t('review.minutes'), review.minutes],
                  [t('review.average'), review.averageRating === null ? '—' : stars(Math.round(review.averageRating))],
                ].map(([label, value]) => (
                  <div key={String(label)} className="rounded-card border border-border bg-surface p-3">
                    <p className="text-[11px] text-fg-muted">{label}</p>
                    <p className="text-[18px] font-semibold tabular-nums text-fg">{value}</p>
                  </div>
                ))}
              </div>
              <div className="flex h-28 items-end gap-1 rounded-card border border-border bg-surface p-3" role="img" aria-label={t('review.monthsLabel')}>
                {review.months.map((n, i) => (
                  <div key={i} className="flex flex-1 flex-col items-center gap-1">
                    <span className="text-[10px] tabular-nums text-fg-muted">{n || ''}</span>
                    <div className="w-full rounded-t bg-primary" style={{ height: `${(n / maxMonth) * 64}px` }} />
                    <span className="text-[10px] text-fg-subtle">{i + 1}</span>
                  </div>
                ))}
              </div>
              {review.pending > 0 && <p className="text-[12px] text-warning">{t('review.pendingNote', { count: review.pending })}</p>}
              {review.items.length === 0 ? (
                <Empty className="py-8" icon={<Clapperboard />} title={t('review.empty', { year: review.year })} />
              ) : (
                <ul className="divide-y divide-border rounded-card border border-border bg-surface text-[12px]">
                  {review.items.map((item, i) => (
                    <li key={`${item.finish}-${i}`} className="flex items-center gap-3 px-3 py-2">
                      <span className="tabular-nums text-fg-muted">{item.finish}</span>
                      <span className="font-medium text-fg">{item.title}</span>
                      <span className="text-fg-muted">{item.creator}</span>
                      <Badge>{t(`kind.${item.kind}`)}</Badge>
                      <span className="ml-auto text-warning">{item.stars}</span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      )}

      {tab === 'pending' && (
        <div className="space-y-2">
          <p className="text-[12px] text-fg-muted">{t('pending.hint')}</p>
          {view.pending.length === 0 ? <Empty className="py-12" icon={<CalendarClock />} title={t('pending.empty')} /> : view.pending.map((s) => sessionRow(s, true))}
        </div>
      )}

      <Drawer
        open={detailId !== null && detail !== null}
        onOpenChange={(open) => {
          if (!open) {
            setDetailId(null)
            setDetail(null)
          }
        }}
        title={detail?.work.title ?? ''}
        closeLabel={t('actions.close')}
      >
        {detail && (
          <div className="space-y-3 p-3 text-[13px]">
            <div className="flex gap-3">
              {cover(detail.work, 'h-28 w-20')}
              <div className="min-w-0 space-y-1 text-[12px]">
                <p className="text-fg">{[t(`kind.${detail.work.kind}`), detail.work.creator, detail.work.year, detail.work.genre].filter(Boolean).join(' · ')}</p>
                {detail.work.total && <p className="text-fg-muted">{t('work.totalLine', { total: detail.work.total, unit: unitOf(detail.work.kind) })}</p>}
                <div className="flex flex-wrap gap-1">
                  {detail.work.tags.map((tg) => (
                    <Badge key={tg}>{tg}</Badge>
                  ))}
                </div>
                {detail.work.notes && <p className="whitespace-pre-wrap text-fg-muted">{detail.work.notes}</p>}
              </div>
            </div>
            <SegmentedControl<Status>
              size="sm"
              value={detail.work.status}
              onValueChange={(s) => void quick({ kind: 'setStatus', workId: detail.work.id, status: s })}
              options={STATUSES.map((s) => ({ value: s, label: statusLabel(s, detail.work.kind) }))}
            />
            <div className="flex flex-wrap gap-1">
              <Button size="sm" variant="primary" disabled={locked} onClick={() => void quick({ kind: 'start', workId: detail.work.id, start: today })}>
                <Play />
                {detail.sessions.length > 0 ? t('session.again', { verb: t(`verb.${detail.work.kind}`) }) : t('session.begin', { verb: t(`verb.${detail.work.kind}`) })}
              </Button>
              <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'work', work: detail.work })}>
                <Pencil />
                {t('work.edit')}
              </Button>
              <Button size="sm" variant="ghost" disabled={locked} onClick={() => void chooseCover(detail.work)}>
                <Camera />
                {detail.work.hasCover ? t('cover.replace') : t('cover.add')}
              </Button>
              {detail.work.hasCover && (
                <Button size="sm" variant="ghost" disabled={locked} onClick={() => void removeCover(detail.work)}>
                  {t('cover.remove')}
                </Button>
              )}
              {detail.sessions.length === 0 && (
                <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'deleteWork', work: detail.work })}>
                  <Trash2 />
                  {t('work.delete')}
                </Button>
              )}
            </div>
            <h3 className="pt-2 text-[12px] font-semibold text-fg">{t('session.history', { count: detail.sessions.length })}</h3>
            {detail.sessions.length === 0 ? <p className="text-[12px] text-fg-muted">{t('session.none')}</p> : <div className="space-y-2">{detail.sessions.map((s) => sessionRow(s, false))}</div>}
          </div>
        )}
      </Drawer>

      {dialog?.kind === 'work' && <WorkForm work={dialog.work} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'session' && <SessionForm session={dialog.session} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'complete' && <CompleteForm session={dialog.session} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'uncomplete' && (
        <ConfirmForm title={t('complete.undo')} message={t('complete.undoHint')} op={{ kind: 'uncomplete', sessionId: dialog.session.id }} onClose={() => setDialog(null)} actions={actions} confirmLabel={t('complete.undo')} />
      )}
      {dialog?.kind === 'deleteSession' && (
        <ConfirmForm title={t('session.delete')} message={t('session.deleteHint')} op={{ kind: 'deleteSession', sessionId: dialog.session.id }} onClose={() => setDialog(null)} actions={actions} confirmLabel={t('session.delete')} danger />
      )}
      {dialog?.kind === 'deleteWork' && (
        <ConfirmForm
          title={t('work.delete')}
          message={t('work.deleteHint', { title: dialog.work.title })}
          op={{ kind: 'deleteWork', workId: dialog.work.id }}
          onClose={() => setDialog(null)}
          actions={{
            ...actions,
            commit: async (op) => {
              const work = dialog.work
              const ok = await actions.commit(op)
              if (ok) {
                if (work.hasCover) await savePluginState(pluginId, coverKey(work.id), null).catch(() => undefined)
                setDetailId(null)
                setDetail(null)
              }
              return ok
            },
          }}
          confirmLabel={t('work.delete')}
          danger
        />
      )}
      {dialog?.kind === 'undo' && view.undo && (
        <ConfirmForm title={t(`undo.${view.undo}`)} message={t('undo.hint')} op={{ kind: 'undo' }} onClose={() => setDialog(null)} actions={actions} confirmLabel={t('undo.confirm')} />
      )}

      <Modal
        open={restore !== null}
        onOpenChange={(open) => !open && setRestore(null)}
        title={t('restore.title')}
        closeLabel={t('actions.close')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setRestore(null)}>
              {t('actions.cancel')}
            </Button>
            <Button variant={restore?.mode === 'replace' ? 'danger' : 'primary'} loading={busy} disabled={!restore?.preview || locked} onClick={() => void confirmRestore()}>
              {restore?.mode === 'replace' ? t('restore.confirmReplace') : t('restore.confirmMerge')}
            </Button>
          </>
        }
      >
        {restore && (
          <div className="space-y-3 text-[13px]">
            <SegmentedControl<'merge' | 'replace'>
              size="sm"
              value={restore.mode}
              onValueChange={(mode) => void previewRestore(restore.text, mode)}
              options={[
                { value: 'merge', label: t('restore.merge') },
                { value: 'replace', label: t('restore.replace') },
              ]}
            />
            <p className="text-[12px] text-fg-muted">{restore.mode === 'merge' ? t('restore.mergeHint') : t('restore.replaceHint')}</p>
            {restore.error && <p className="text-[12px] text-danger">{restore.error}</p>}
            {!restore.preview && !restore.error && <Spinner />}
            {restore.preview && (
              <ul className="space-y-1 rounded-card border border-border bg-bg p-3 text-[12px]">
                <li>{t('restore.added', { works: restore.preview.report.worksAdded, sessions: restore.preview.report.sessionsAdded, covers: restore.preview.report.covers })}</li>
                {restore.mode === 'merge' && <li>{t('restore.skipped', { works: restore.preview.report.worksSkipped, sessions: restore.preview.report.sessionsSkipped })}</li>}
                {restore.mode === 'replace' && <li className="text-danger">{t('restore.removed', { works: restore.preview.report.worksRemoved, sessions: restore.preview.report.sessionsRemoved })}</li>}
              </ul>
            )}
          </div>
        )}
      </Modal>
    </div>
  )
}
