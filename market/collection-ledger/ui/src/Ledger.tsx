import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Drawer,
  Empty,
  Input,
  Modal,
  Pagination,
  SegmentedControl,
  Select,
  Spinner,
  Tabs,
  Tooltip,
  toast,
} from '@toolforge/ui'
import { callPlugin, host, loadPluginState, savePluginState, usePlugin } from '@toolforge/plugin-ui-sdk'
import {
  Archive,
  ArchiveRestore,
  Camera,
  Download,
  FileJson,
  Handshake,
  ListChecks,
  Package,
  Pencil,
  Plus,
  RotateCcw,
  Search,
  SlidersHorizontal,
  Trash2,
  Undo2,
  Upload,
} from 'lucide-react'
import { ConfirmForm, ItemForm, LendForm, OwnedForm, ReturnForm, type Actions } from './Forms'
import { localDate, newOpId } from './today'
import type { Impact, ItemState, ItemView, Ledger as LedgerData, LoanView, Op, Outcome, Restored, View } from './types'

const LEDGER_KEY = 'ledger'
const photoKey = (id: string) => `photo:${id}`
const PAGE = 20

type Dialog =
  | { kind: 'item'; item: ItemView | null }
  | { kind: 'owned'; item: ItemView }
  | { kind: 'lend'; item: ItemView }
  | { kind: 'return'; loan: LoanView }
  | { kind: 'archive'; item: ItemView }
  | { kind: 'delete'; item: ItemView }
  | { kind: 'undo' }
  | null

const DUE_VARIANT = { overdue: 'danger', soon: 'warning', ok: 'neutral', none: 'neutral' } as const

export function Ledger() {
  const { t, manifest, errorMessage } = usePlugin()
  const pluginId = manifest.id
  const [ledger, setLedger] = useState<LedgerData | null>(null)
  const [view, setView] = useState<View | null>(null)
  /** 已算出但还没保存成功的台账：保存失败时保留，可重试，期间不允许其他修改 */
  const [unsaved, setUnsaved] = useState<LedgerData | null>(null)
  const [busy, setBusy] = useState(false)
  const [tab, setTab] = useState<'items' | 'pending'>('items')
  const [search, setSearch] = useState('')
  const [location, setLocation] = useState('')
  const [category, setCategory] = useState('')
  const [state, setState] = useState<ItemState>('all')
  const [page, setPage] = useState(1)
  const [pendingPage, setPendingPage] = useState(1)
  const [dialog, setDialog] = useState<Dialog>(null)
  const [detailId, setDetailId] = useState<string | null>(null)
  const [detail, setDetail] = useState<{ item: ItemView; loans: LoanView[] } | null>(null)
  const [photos, setPhotos] = useState<Record<string, string>>({})
  const [restore, setRestore] = useState<{ text: string; mode: 'merge' | 'replace'; preview: Restored | null; error: string | null } | null>(null)
  const ledgerRef = useRef<LedgerData | null>(null)
  const today = localDate()

  const show = (next: LedgerData) => {
    ledgerRef.current = next
    setLedger(next)
  }

  // 首次加载：已有台账或新建空台账（结构由 Rust 给出）
  useEffect(() => {
    let alive = true
    loadPluginState<LedgerData>(pluginId, LEDGER_KEY)
      .then(async (saved) => saved ?? (await callPlugin<LedgerData>(pluginId, 'empty', {})))
      .then((value) => alive && show(value))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [pluginId, errorMessage])

  // 视图：台账或筛选变化时由 Rust 重新计算
  useEffect(() => {
    if (!ledger) return
    let alive = true
    callPlugin<View>(pluginId, 'view', {
      ledger,
      today,
      search,
      location: location || null,
      category: category || null,
      state,
      offset: (page - 1) * PAGE,
      limit: PAGE,
      pendingOffset: (pendingPage - 1) * PAGE,
      pendingLimit: PAGE,
    })
      .then((v) => alive && setView(v))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [pluginId, ledger, today, search, location, category, state, page, pendingPage, errorMessage])

  // 详情抽屉：选中藏品的往来历史
  useEffect(() => {
    if (!ledger || !detailId) return
    let alive = true
    callPlugin<{ item: ItemView; loans: LoanView[] }>(pluginId, 'history', { ledger, itemId: detailId, today })
      .then((d) => alive && setDetail(d))
      .catch(() => alive && setDetail(null))
    return () => {
      alive = false
    }
  }, [pluginId, ledger, detailId, today])

  // 照片按需读取（只读当前页与详情里用到的）
  const wanted = [...(view?.items ?? []), ...(detail ? [detail.item] : [])].filter((i) => i.hasPhoto && photos[i.id] === undefined).map((i) => i.id)
  const wantedKey = wanted.join(',')
  useEffect(() => {
    if (!wantedKey) return
    let alive = true
    Promise.all(wantedKey.split(',').map(async (id) => [id, (await loadPluginState<string>(pluginId, photoKey(id))) ?? ''] as const))
      .then((entries) => alive && setPhotos((p) => ({ ...p, ...Object.fromEntries(entries) })))
      .catch(() => undefined)
    return () => {
      alive = false
    }
  }, [pluginId, wantedKey])

  /** 保存台账；成功才更新界面 */
  const persist = useCallback(
    async (next: LedgerData) => {
      try {
        await savePluginState(pluginId, LEDGER_KEY, next)
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

  const run = async (op: Op, opId: string, preview: boolean) => {
    const current = ledgerRef.current
    if (!current) throw new Error('not loaded')
    return callPlugin<Outcome>(pluginId, 'apply', { ledger: current, op, opId, revision: current.revision, today, preview })
  }

  const actions: Actions = {
    busy: busy || unsaved !== null,
    preview: async (op) => (await run(op, newOpId(), true)).impact as Impact,
    commit: async (op) => {
      if (unsaved) return false
      setBusy(true)
      try {
        const out = await run(op, newOpId(), false)
        if (!out.ledger) return false
        const saved = await persist(out.ledger)
        if (saved && !out.duplicate) toast.success(t(`done.${op.kind}`))
        return saved
      } catch (error) {
        toast.error(errorMessage(error))
        return false
      } finally {
        setBusy(false)
      }
    },
  }

  const retrySave = async () => {
    if (!unsaved) return
    setBusy(true)
    if (await persist(unsaved)) toast.success(t('save.retried'))
    setBusy(false)
  }

  const choosePhoto = async (item: ItemView) => {
    try {
      const path = await host.dialog.openFile([{ name: t('photo.images'), extensions: ['jpg', 'jpeg', 'png', 'webp'] }])
      if (!path) return
      setBusy(true)
      const { data } = await callPlugin<{ data: string }>(pluginId, 'photo', { path })
      // 先写照片，再更新标记；标记保存失败时照片键无人引用，下次更换会覆盖
      await savePluginState(pluginId, photoKey(item.id), data)
      setPhotos((p) => ({ ...p, [item.id]: data }))
      setBusy(false)
      await actions.commit({ kind: 'setPhoto', itemId: item.id, hasPhoto: true })
    } catch (error) {
      setBusy(false)
      toast.error(errorMessage(error))
    }
  }

  const removePhoto = async (item: ItemView) => {
    if (await actions.commit({ kind: 'setPhoto', itemId: item.id, hasPhoto: false })) {
      await savePluginState(pluginId, photoKey(item.id), null).catch(() => undefined)
      setPhotos((p) => ({ ...p, [item.id]: '' }))
    }
  }

  const afterDelete = async (item: ItemView) => {
    if (item.hasPhoto) await savePluginState(pluginId, photoKey(item.id), null).catch(() => undefined)
    setDetailId(null)
    setDetail(null)
  }

  const exportList = async () => {
    const current = ledgerRef.current
    if (!current) return
    try {
      const headers = ['name', 'series', 'edition', 'code', 'category', 'location', 'condition', 'owned', 'available', 'outstanding', 'archived', 'notes'].map((k) => t(`csv.${k}`))
      const { content } = await callPlugin<{ content: string }>(pluginId, 'exportList', { ledger: current, headers })
      const path = await host.dialog.saveFile(`collection-${today}.csv`, [{ name: 'CSV', extensions: ['csv'] }])
      if (!path) return
      await host.fs.writeText(path, content)
      toast.success(t('export.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const exportBackup = async () => {
    const current = ledgerRef.current
    if (!current) return
    try {
      const withPhoto = current.items.filter((i) => i.hasPhoto).map((i) => i.id)
      const loaded = await Promise.all(withPhoto.map(async (id) => [id, (await loadPluginState<string>(pluginId, photoKey(id))) ?? ''] as const))
      const all = Object.fromEntries(loaded.filter(([, data]) => data))
      const { content } = await callPlugin<{ content: string }>(pluginId, 'exportBackup', { ledger: current, photos: all, today })
      const path = await host.dialog.saveFile(`collection-backup-${today}.json`, [{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await host.fs.writeText(path, content)
      toast.success(t('export.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const previewRestore = async (text: string, mode: 'merge' | 'replace') => {
    const current = ledgerRef.current
    if (!current) return
    setRestore({ text, mode, preview: null, error: null })
    try {
      const preview = await callPlugin<Restored>(pluginId, 'restore', { ledger: current, text, mode, revision: current.revision, preview: true })
      setRestore({ text, mode, preview, error: null })
    } catch (error) {
      setRestore({ text, mode, preview: null, error: errorMessage(error) })
    }
  }

  const openRestore = async () => {
    try {
      const path = await host.dialog.openFile([{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      const text = await host.fs.readText(path)
      await previewRestore(text, 'merge')
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 执行恢复：先写入新照片，再保存台账，最后删掉被替换藏品的照片 */
  const confirmRestore = async () => {
    const current = ledgerRef.current
    if (!restore || !current || unsaved) return
    setBusy(true)
    try {
      const out = await callPlugin<Restored>(pluginId, 'restore', { ledger: current, text: restore.text, mode: restore.mode, revision: current.revision, preview: false })
      if (!out.ledger) return
      for (const [id, data] of Object.entries(out.photos)) await savePluginState(pluginId, photoKey(id), data)
      if (!(await persist(out.ledger))) return
      const kept = new Set(Object.keys(out.photos))
      for (const id of out.removePhotos.filter((id) => !kept.has(id))) await savePluginState(pluginId, photoKey(id), null).catch(() => undefined)
      setPhotos({})
      setRestore(null)
      toast.success(t('restore.done'))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  if (!ledger || !view) {
    return (
      <div className="flex h-full items-center justify-center p-3">
        <Spinner />
      </div>
    )
  }

  const locked = busy || unsaved !== null
  const s = view.summary
  const optionList = (values: string[], all: string) => [{ value: '', label: all }, ...values.map((v) => ({ value: v, label: v }))]

  const itemCard = (item: ItemView) => (
    <div key={item.id} className="flex gap-3 rounded-card border border-border bg-surface p-3">
      {item.hasPhoto && photos[item.id] ? (
        <img src={photos[item.id]} alt="" className="size-14 shrink-0 rounded-control object-cover" />
      ) : (
        <span className="flex size-14 shrink-0 items-center justify-center rounded-control bg-bg text-fg-subtle">
          <Package className="size-5" />
        </span>
      )}
      <div className="min-w-0 flex-1 space-y-1">
        <button type="button" onClick={() => setDetailId(item.id)} className="block max-w-full truncate text-left text-[13px] font-semibold text-fg hover:underline">
          {item.name}
        </button>
        <p className="truncate text-[11px] text-fg-muted">{[item.series, item.edition, item.code].filter(Boolean).join(' · ') || '—'}</p>
        <div className="flex flex-wrap gap-1">
          {item.location && <Badge>{item.location}</Badge>}
          {item.category && <Badge>{item.category}</Badge>}
          {item.condition && <Badge>{item.condition}</Badge>}
          {item.archived && <Badge variant="neutral">{t('item.archived')}</Badge>}
        </div>
        <p className="text-[12px] tabular-nums text-fg">
          {t('counts.line', { owned: item.owned, available: item.available, outstanding: item.outstanding })}
        </p>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1">
        <Button size="sm" variant="primary" disabled={locked || item.archived || item.available === 0} onClick={() => setDialog({ kind: 'lend', item })}>
          <Handshake />
          {t('lend.action')}
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setDetailId(item.id)}>
          {t('item.details')}
        </Button>
      </div>
    </div>
  )

  const loanRow = (loan: LoanView, showItem: boolean) => (
    <div key={loan.id} className="space-y-1 rounded-card border border-border bg-surface p-3 text-[12px]">
      <div className="flex flex-wrap items-center gap-2">
        {showItem && <span className="font-semibold text-fg">{loan.itemName}</span>}
        <span className="text-fg">{t('loan.borrower', { name: loan.borrower })}</span>
        {loan.outstanding > 0 && loan.dueState !== 'none' && (
          <Badge variant={DUE_VARIANT[loan.dueState]}>
            {loan.dueState === 'overdue' ? t('due.overdue', { days: -(loan.daysLeft ?? 0) }) : t(`due.${loan.dueState}`, { days: loan.daysLeft ?? 0 })}
          </Badge>
        )}
        {loan.outstanding === 0 && <Badge variant="success">{t('loan.closed')}</Badge>}
        <div className="ml-auto">
          {loan.outstanding > 0 && (
            <Button size="sm" variant="secondary" disabled={locked} onClick={() => setDialog({ kind: 'return', loan })}>
              <RotateCcw />
              {t('return.action')}
            </Button>
          )}
        </div>
      </div>
      <p className="tabular-nums text-fg-muted">
        {t('loan.line', { qty: loan.qty, returned: loan.returned, outstanding: loan.outstanding, date: loan.date })}
        {loan.due ? ` · ${t('loan.due', { date: loan.due })}` : ''}
      </p>
      {loan.note && <p className="text-fg-muted">{loan.note}</p>}
      {loan.returns.length > 0 && (
        <ul className="space-y-0.5 border-l-2 border-border pl-2 text-fg-muted">
          {loan.returns.map((r) => (
            <li key={r.id}>{t('loan.returnLine', { qty: r.qty, date: r.date, note: r.note ? ` · ${r.note}` : '' })}</li>
          ))}
        </ul>
      )}
    </div>
  )

  const pagination = (current: number, total: number, onChange: (p: number) => void) =>
    total > PAGE && (
      <Pagination
        page={current}
        pageSize={PAGE}
        total={total}
        onPageChange={onChange}
        prevLabel={t('page.prev')}
        nextLabel={t('page.next')}
        pageLabel={(p) => t('page.number', { page: p })}
        totalLabel={t('page.total', { count: total })}
      />
    )

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
        <Badge>{t('summary.items', { count: s.items })}</Badge>
        <Badge>{t('summary.owned', { count: s.owned })}</Badge>
        <Badge variant="success">{t('summary.available', { count: s.available })}</Badge>
        <Badge variant="info">{t('summary.outstanding', { count: s.outstanding })}</Badge>
        {s.overdue > 0 && <Badge variant="danger">{t('summary.overdue', { count: s.overdue })}</Badge>}
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
          <Button size="sm" variant="primary" disabled={locked} onClick={() => setDialog({ kind: 'item', item: null })}>
            <Plus />
            {t('item.new')}
          </Button>
        </div>
      </div>

      <Tabs
        value={tab}
        onValueChange={setTab}
        items={[
          { value: 'items', label: t('tabs.items'), icon: <Package /> },
          { value: 'pending', label: t('tabs.pending', { count: view.pendingTotal }), icon: <ListChecks /> },
        ]}
      />

      {tab === 'items' ? (
        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <Input
              size="sm"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value)
                setPage(1)
              }}
              placeholder={t('filter.search')}
              aria-label={t('filter.search')}
              leading={<Search />}
              wrapperClassName="min-w-48 flex-1"
            />
            <Select<string> size="sm" className="w-36" value={location} onValueChange={(v) => { setLocation(v); setPage(1) }} options={optionList(view.locations, t('filter.allLocations'))} />
            <Select<string> size="sm" className="w-36" value={category} onValueChange={(v) => { setCategory(v); setPage(1) }} options={optionList(view.categories, t('filter.allCategories'))} />
            <SegmentedControl<ItemState>
              size="sm"
              value={state}
              onValueChange={(v) => {
                setState(v)
                setPage(1)
              }}
              options={(['all', 'inHand', 'lent', 'archived'] as const).map((value) => ({ value, label: t(`filter.${value}`) }))}
            />
          </div>
          {view.items.length === 0 ? (
            <Empty className="py-12" icon={<Package />} title={ledger.items.length === 0 ? t('empty.none') : t('empty.filtered')} />
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(320px,1fr))] gap-2">{view.items.map(itemCard)}</div>
          )}
          {pagination(page, view.total, setPage)}
        </div>
      ) : (
        <div className="space-y-2">
          <p className="text-[12px] text-fg-muted">{t('pending.hint')}</p>
          {view.pending.length === 0 ? <Empty className="py-12" icon={<ListChecks />} title={t('pending.empty')} /> : view.pending.map((loan) => loanRow(loan, true))}
          {pagination(pendingPage, view.pendingTotal, setPendingPage)}
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
        title={detail?.item.name ?? ''}
        closeLabel={t('actions.close')}
      >
        {detail && (
          <div className="space-y-3 p-3 text-[13px]">
            {detail.item.hasPhoto && photos[detail.item.id] && <img src={photos[detail.item.id]} alt="" className="max-h-56 w-full rounded-card object-contain" />}
            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[12px]">
              {(['series', 'edition', 'code', 'category', 'location', 'condition', 'notes'] as const).map((key) =>
                detail.item[key] ? (
                  <div key={key} className="contents">
                    <dt className="text-fg-muted">{t(`item.${key}`)}</dt>
                    <dd className="break-words text-fg">{detail.item[key]}</dd>
                  </div>
                ) : null,
              )}
              <dt className="text-fg-muted">{t('item.created')}</dt>
              <dd className="text-fg">{detail.item.created}</dd>
            </dl>
            <p className="tabular-nums text-fg">{t('counts.line', { owned: detail.item.owned, available: detail.item.available, outstanding: detail.item.outstanding })}</p>
            <div className="flex flex-wrap gap-1">
              <Button size="sm" variant="primary" disabled={locked || detail.item.archived || detail.item.available === 0} onClick={() => setDialog({ kind: 'lend', item: detail.item })}>
                <Handshake />
                {t('lend.action')}
              </Button>
              <Button size="sm" variant="secondary" disabled={locked} onClick={() => setDialog({ kind: 'owned', item: detail.item })}>
                <SlidersHorizontal />
                {t('owned.action')}
              </Button>
              <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'item', item: detail.item })}>
                <Pencil />
                {t('item.edit')}
              </Button>
              <Button size="sm" variant="ghost" disabled={locked} onClick={() => void choosePhoto(detail.item)}>
                <Camera />
                {detail.item.hasPhoto ? t('photo.replace') : t('photo.add')}
              </Button>
              {detail.item.hasPhoto && (
                <Button size="sm" variant="ghost" disabled={locked} onClick={() => void removePhoto(detail.item)}>
                  {t('photo.remove')}
                </Button>
              )}
              <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'archive', item: detail.item })}>
                {detail.item.archived ? <ArchiveRestore /> : <Archive />}
                {detail.item.archived ? t('archive.restore') : t('archive.action')}
              </Button>
              {!detail.item.hasLoans && (
                <Button size="sm" variant="ghost" disabled={locked} onClick={() => setDialog({ kind: 'delete', item: detail.item })}>
                  <Trash2 />
                  {t('delete.action')}
                </Button>
              )}
            </div>
            <h3 className="pt-2 text-[12px] font-semibold text-fg">{t('history.title', { count: detail.loans.length })}</h3>
            {detail.loans.length === 0 ? <p className="text-[12px] text-fg-muted">{t('history.empty')}</p> : <div className="space-y-2">{detail.loans.map((loan) => loanRow(loan, false))}</div>}
          </div>
        )}
      </Drawer>

      {dialog?.kind === 'item' && <ItemForm item={dialog.item} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'owned' && <OwnedForm item={dialog.item} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'lend' && <LendForm item={dialog.item} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'return' && <ReturnForm loan={dialog.loan} onClose={() => setDialog(null)} actions={actions} />}
      {dialog?.kind === 'archive' && (
        <ConfirmForm
          title={dialog.item.archived ? t('archive.restore') : t('archive.action')}
          message={dialog.item.archived ? t('archive.restoreHint') : t('archive.hint')}
          op={{ kind: 'archive', itemId: dialog.item.id, archived: !dialog.item.archived }}
          onClose={() => setDialog(null)}
          actions={actions}
          confirmLabel={dialog.item.archived ? t('archive.restore') : t('archive.action')}
        />
      )}
      {dialog?.kind === 'delete' && (
        <ConfirmForm
          title={t('delete.action')}
          message={t('delete.hint', { name: dialog.item.name })}
          op={{ kind: 'delete', itemId: dialog.item.id }}
          onClose={() => setDialog(null)}
          actions={{
            ...actions,
            commit: async (op) => {
              const item = dialog.item
              const ok = await actions.commit(op)
              if (ok) await afterDelete(item)
              return ok
            },
          }}
          confirmLabel={t('delete.confirm')}
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
                <li>{t('restore.added', { items: restore.preview.report.itemsAdded, loans: restore.preview.report.loansAdded, photos: restore.preview.report.photos })}</li>
                {restore.mode === 'merge' && <li>{t('restore.skipped', { items: restore.preview.report.itemsSkipped, loans: restore.preview.report.loansSkipped })}</li>}
                {restore.mode === 'replace' && <li className="text-danger">{t('restore.removed', { items: restore.preview.report.itemsRemoved, loans: restore.preview.report.loansRemoved })}</li>}
              </ul>
            )}
          </div>
        )}
      </Modal>
    </div>
  )
}
