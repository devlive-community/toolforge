import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Empty,
  NumberInput,
  Panel,
  Spinner,
  Tooltip,
  cn,
  toast,
} from '@toolforge/ui'
import { host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import {
  ClipboardCopy,
  Download,
  History,
  Minus,
  PackageOpen,
  Pencil,
  Plus,
  Snowflake,
  Trash2,
  Undo2,
} from 'lucide-react'
import {
  compatibleUnits,
  formatAmount,
  todayText,
  type ApplyOut,
  type Batch,
  type BatchView,
  type Location,
  type Movement,
  type MovementKind,
  type ReviewOut,
  type Settings,
  type UnitId,
  UNITS,
  MAX_BATCHES,
  MAX_SOON_DAYS,
} from './types'

type CardAction = 'consume' | 'discard' | 'adjust' | 'dates' | 'detail' | null

interface Draft {
  name: string
  location: string
  intake: number
  unit: UnitId
  purchasedDate: string
  labelDate: string
}

const inputClass =
  'h-9 w-full rounded-control border border-border bg-surface px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring'

export function Fridge() {
  const { t, call, errorMessage } = usePlugin()
  const [batches, setBatches] = usePluginState<Batch[]>('batches', [])
  const [movements, setMovements] = usePluginState<Movement[]>('movements', [])
  const [settings, setSettings] = usePluginState<Settings>('settings', {
    soonDays: 3,
    locations: [
      { id: 'fridge', name: 'fridge' },
      { id: 'freezer', name: 'freezer' },
      { id: 'pantry', name: 'pantry' },
    ],
  })

  const [review, setReview] = useState<ReviewOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [locationFilter, setLocationFilter] = useState<string>('all')
  const [openedOnly, setOpenedOnly] = useState(false)
  const [draft, setDraft] = useState<Draft>({
    name: '',
    location: 'fridge',
    intake: 1,
    unit: 'pack',
    purchasedDate: todayText(),
    labelDate: '',
  })
  const [cardAction, setCardAction] = useState<{ key: string; action: CardAction }>({ key: '', action: null })
  const [opForm, setOpForm] = useState<{ amount: number; unit: UnitId; kind: MovementKind }>({
    amount: 1,
    unit: 'pack',
    kind: 'consume',
  })
  const [dateForm, setDateForm] = useState<{ labelDate: string; attentionDate: string; openedDate: string }>({
    labelDate: '',
    attentionDate: '',
    openedDate: '',
  })
  const [newLocation, setNewLocation] = useState('')
  const today = todayText()
  const seqRef = useRef(0)

  const locationName = useCallback(
    (id: string) => {
      const known = settings.locations.find((l) => l.id === id)
      if (!known) return t('common.removed')
      return ['fridge', 'freezer', 'pantry'].includes(known.id) ? t(`location.${known.id}`) : known.name
    },
    [settings.locations, t],
  )

  // 汇总与日期分档全部在 Rust 计算
  useEffect(() => {
    const seq = ++seqRef.current
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const out = await call<ReviewOut>('review', {
            batches,
            movements,
            today,
            soonDays: settings.soonDays,
          })
          if (seq !== seqRef.current) return
          setReview(out)
        } catch (error) {
          if (seq === seqRef.current) toast.error(errorMessage(error))
        }
      })()
    }, 60)
    return () => clearTimeout(timer)
  }, [batches, call, errorMessage, movements, settings.soonDays, today])

  /** 数量事务：Rust 校验通过才落账；重复提交在等待期由禁用按钮挡住 */
  const runApply = async (op: object) => {
    const out = await call<ApplyOut>('apply', { batches, movements, op, today })
    if (out.movement) setMovements([...movements, out.movement])
    if (out.batch) setBatches(batches.map((b) => (b.id === out.batch!.id ? out.batch! : b)))
  }

  const act = async (fn: () => Promise<void>) => {
    setBusy(true)
    try {
      await fn()
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  const addBatch = () =>
    act(async () => {
      const name = draft.name.trim()
      if (!name) {
        toast.error(t('errors.fridge.name_required'))
        return
      }
      if (batches.length >= MAX_BATCHES) {
        toast.error(t('errors.fridge.too_many_batches', { limit: MAX_BATCHES }))
        return
      }
      if (!(draft.intake > 0) || draft.purchasedDate === '') {
        toast.error(t('errors.fridge.bad_amount'))
        return
      }
      const batch: Batch = {
        id: crypto.randomUUID(),
        name,
        location: draft.location,
        unit: draft.unit,
        intake: draft.intake,
        purchasedDate: draft.purchasedDate,
        ...(draft.labelDate ? { labelDate: draft.labelDate } : {}),
      }
      setBatches([...batches, batch])
      setDraft((prev) => ({ ...prev, name: '', labelDate: '' }))
    })

  const openAction = (view: BatchView, action: Exclude<CardAction, null>) => {
    // 再次点击同一动作 = 收起
    if (cardAction.key === view.id && cardAction.action === action) {
      setCardAction({ key: '', action: null })
      return
    }
    const unit = (view.unit as UnitId) ?? 'piece'
    setOpForm({
      amount: 1,
      unit: (compatibleUnits(view.unit)[0] ?? unit) as UnitId,
      kind: action === 'discard' ? 'discard' : 'consume',
    })
    setDateForm({
      labelDate: view.labelDate ?? '',
      attentionDate: view.attentionDate ?? '',
      openedDate: view.openedDate ?? '',
    })
    setCardAction({ key: view.id, action })
  }

  const submitDeduct = (view: BatchView, kind: MovementKind) =>
    act(async () => {
      await runApply({
        type: 'deduct',
        movementId: crypto.randomUUID(),
        batchId: view.id,
        kind,
        amount: opForm.amount,
        unit: opForm.unit,
        date: today,
      })
      setCardAction({ key: '', action: null })
    })

  const submitAdjust = (view: BatchView) =>
    act(async () => {
      await runApply({
        type: 'setIntake',
        batchId: view.id,
        intake: opForm.amount,
        unit: opForm.unit,
        date: today,
        movementId: crypto.randomUUID(),
      })
      setCardAction({ key: '', action: null })
    })

  /** 开封 / 更正开封：只记录日期，不扣数量 */
  const setOpenState = (view: BatchView, openedDate?: string) =>
    act(async () => {
      setBatches(
        batches.map((b) =>
          b.id === view.id
            ? {
                ...b,
                ...(openedDate ? { openedDate } : { openedDate: undefined }),
              }
            : b,
        ),
      )
      setCardAction({ key: '', action: null })
    })

  const saveDates = (view: BatchView) =>
    act(async () => {
      setBatches(
        batches.map((b) =>
          b.id === view.id
            ? {
                ...b,
                ...(dateForm.labelDate ? { labelDate: dateForm.labelDate } : { labelDate: undefined }),
                ...(dateForm.attentionDate
                  ? { attentionDate: dateForm.attentionDate }
                  : { attentionDate: undefined }),
              }
            : b,
        ),
      )
      setCardAction({ key: '', action: null })
    })

  /** 撤销该批最近一笔变动 */
  const undoLast = (view: BatchView) =>
    act(async () => {
      if (!view.lastMovementId) return
      setMovements(movements.filter((m) => m.id !== view.lastMovementId))
    })

  const deleteBatch = (view: BatchView) =>
    act(async () => {
      setBatches(batches.filter((b) => b.id !== view.id))
      setMovements(movements.filter((m) => m.batchId !== view.id))
    })

  const addLocation = () => {
    const name = newLocation.trim()
    if (!name || settings.locations.some((l) => l.name === name)) return
    setSettings({ ...settings, locations: [...settings.locations, { id: crypto.randomUUID(), name }] })
    setNewLocation('')
  }

  const removeLocation = (id: string) => {
    if (['fridge', 'freezer', 'pantry'].includes(id)) return
    if (batches.some((b) => b.location === id)) {
      toast.error(t('errors.fridge.location_in_use'))
      return
    }
    setSettings({ ...settings, locations: settings.locations.filter((l) => l.id !== id) })
  }

  /** 分组：逾期 / 临近 / 未知日期 / 正常 / 已用完；组内按标签日期升序（未知靠后） */
  const groups = useMemo(() => {
    const all = review?.batches ?? []
    const filtered = all.filter(
      (view) =>
        (locationFilter === 'all' || view.location === locationFilter) &&
        (!openedOnly || view.opened),
    )
    const active = filtered.filter((view) => view.status === 'active')
    const byDate = (a: BatchView, b: BatchView) =>
      (a.labelDate ?? '9999-12-31').localeCompare(b.labelDate ?? '9999-12-31') || a.name.localeCompare(b.name)
    return [
      { key: 'overdue', list: active.filter((v) => v.labelState === 'overdue').sort(byDate) },
      { key: 'soon', list: active.filter((v) => v.labelState === 'soon').sort(byDate) },
      { key: 'unknown', list: active.filter((v) => v.labelState === 'unknown').sort(byDate) },
      { key: 'ok', list: active.filter((v) => v.labelState === 'ok').sort(byDate) },
      { key: 'depleted', list: filtered.filter((v) => v.status === 'depleted').sort(byDate) },
    ]
  }, [locationFilter, openedOnly, review])

  const listText = useMemo(() => {
    const lines: string[] = [t('table.title', { date: today })]
    for (const group of groups) {
      if (group.list.length === 0) continue
      lines.push(t(`group.${group.key}`))
      for (const view of group.list) {
        lines.push(
          `- ${view.name}（${locationName(view.location)}）${formatAmount(view.remaining)}/${formatAmount(view.intake)} ${t(`unit.${view.unit}`)} · ${t(
            `label.${view.labelState}`,
            { date: view.labelDate ?? '—' },
          )}${view.opened ? ` · ${t('batch.openedOn', { date: view.openedDate ?? '' })}` : ''}`,
        )
      }
    }
    return lines.join('\n')
  }, [groups, locationName, t, today])

  const copyList = async () => {
    try {
      await host.clipboard.writeText(listText)
      toast.success(t('table.copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const saveList = async () => {
    try {
      const path = await host.dialog.saveFile(`fridge-${today}.txt`, [{ name: 'TXT', extensions: ['txt'] }])
      if (!path) return
      await host.fs.writeText(path, listText)
      toast.success(t('table.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const saveBackup = async () => {
    try {
      const path = await host.dialog.saveFile(`fridge-backup-${today}.json`, [
        { name: 'JSON', extensions: ['json'] },
      ])
      if (!path) return
      await host.fs.writeText(path, JSON.stringify({ batches, movements, settings }, null, 2))
      toast.success(t('table.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const renderUnitSelect = (value: UnitId, onChange: (unit: UnitId) => void, batchUnit: string) => (
    <select
      value={value}
      onChange={(event) => onChange(event.target.value as UnitId)}
      className="h-9 rounded-control border border-border bg-surface px-2 text-sm"
      aria-label={t('batch.unit')}
    >
      {compatibleUnits(batchUnit).map((unit) => (
        <option key={unit} value={unit}>
          {t(`unit.${unit}`)}
        </option>
      ))}
    </select>
  )

  const renderCard = (view: BatchView) => {
    const active = cardAction.key === view.id ? cardAction.action : null
    const isOpenAction = active === 'consume' || active === 'discard'
    return (
      <li key={view.id} className="rounded-card border border-border p-2.5">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="min-w-0 flex-1 truncate text-sm font-medium">{view.name}</span>
          <Badge>{locationName(view.location)}</Badge>
          {view.opened && <Badge variant="info">{t('batch.openedBadge')}</Badge>}
          {view.labelDate ? (
            <Badge
              variant={view.labelState === 'overdue' ? 'danger' : view.labelState === 'soon' ? 'warning' : 'success'}
            >
              {t('batch.labelOn', { date: view.labelDate })}
            </Badge>
          ) : (
            <Badge>{t('label.unknown')}</Badge>
          )}
          <Badge variant="info">
            {t('batch.remaining', {
              remaining: formatAmount(Math.max(view.remaining, 0)),
              intake: formatAmount(view.intake),
              unit: t(`unit.${view.unit}`),
            })}
          </Badge>
        </div>
        {view.attentionDate && (
          <p className="mt-1 text-[11px] text-fg-subtle">
            {t('batch.attentionOn', {
              date: view.attentionDate,
              state: view.attentionState === 'none' ? '' : t(`label.${view.attentionState}`),
            })}
          </p>
        )}
        <div className="mt-2 flex flex-wrap items-center gap-1">
          {view.status === 'active' && (
            <>
              <Button
                size="sm"
                variant="outline"
                onClick={() => openAction(view, 'consume')}
              >
                <Minus />
                {t('batch.consume')}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={() => openAction(view, 'discard')}
              >
                <Trash2 />
                {t('batch.discard')}
              </Button>
            </>
          )}
          <Button
            size="sm"
            variant="outline"
            onClick={() => openAction(view, 'adjust')}
          >
            <Pencil />
            {t('batch.adjustIntake')}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              act(async () => {
                await runApply({
                  type: 'setIntake',
                  batchId: view.id,
                  intake: view.intake,
                  unit: view.unit,
                  date: today,
                  movementId: crypto.randomUUID(),
                })
              })
            }
            disabled={busy}
          >
            <PackageOpen />
            {view.opened ? t('batch.unopen') : t('batch.open')}
          </Button>
          <Tooltip content={t('batch.editDates')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('batch.editDates')}
              onClick={() => openAction(view, 'dates')}
            >
              <Pencil />
            </Button>
          </Tooltip>
          <Tooltip content={t('batch.detail')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('batch.detail')}
              onClick={() => openAction(view, 'detail')}
            >
              <History />
            </Button>
          </Tooltip>
          {view.lastMovementId && (
            <Tooltip content={t('batch.undoLast')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('batch.undoLast')} onClick={() => undoLast(view)}>
                <Undo2 />
              </Button>
            </Tooltip>
          )}
          <Tooltip content={t('batch.delete')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('batch.delete')} onClick={() => deleteBatch(view)}>
              <Trash2 />
            </Button>
          </Tooltip>
        </div>

        {isOpenAction && (
          <div className="mt-2 flex flex-wrap items-center gap-2 rounded-card border border-border-strong bg-surface p-2">
            <NumberInput
              value={opForm.amount}
              onValueChange={(amount) => setOpForm((prev) => ({ ...prev, amount }))}
              min={0}
              step={view.unit === 'g' || view.unit === 'ml' ? 50 : 1}
              className="w-28"
              aria-label={t('batch.amount')}
            />
            {renderUnitSelect(opForm.unit, (unit) => setOpForm((prev) => ({ ...prev, unit })), view.unit)}
            <Button
              size="sm"
              variant="primary"
              disabled={busy}
              onClick={() => submitDeduct(view, active === 'consume' ? 'consume' : 'discard')}
            >
              {t('common.confirm')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setCardAction({ key: '', action: null })}>
              {t('common.cancel')}
            </Button>
          </div>
        )}

        {active === 'adjust' && (
          <div className="mt-2 flex flex-wrap items-center gap-2 rounded-card border border-border-strong bg-surface p-2">
            <NumberInput
              value={opForm.amount}
              onValueChange={(amount) => setOpForm((prev) => ({ ...prev, amount }))}
              min={0}
              step={view.unit === 'g' || view.unit === 'ml' ? 50 : 1}
              className="w-28"
              aria-label={t('batch.intake')}
            />
            {renderUnitSelect(opForm.unit, (unit) => setOpForm((prev) => ({ ...prev, unit })), view.unit)}
            <Button size="sm" variant="primary" disabled={busy} onClick={() => submitAdjust(view)}>
              {t('common.confirm')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setCardAction({ key: '', action: null })}>
              {t('common.cancel')}
            </Button>
            <p className="w-full text-[11px] text-fg-subtle">{t('batch.adjustHint')}</p>
          </div>
        )}

        {active === 'dates' && (
          <div className="mt-2 space-y-2 rounded-card border border-border-strong bg-surface p-2">
            <div className="grid grid-cols-3 gap-2">
              <label className="space-y-1">
                <span className="block text-[11px] text-fg-muted">{t('batch.labelDate')}</span>
                <input
                  type="date"
                  value={dateForm.labelDate}
                  onChange={(event) => setDateForm((prev) => ({ ...prev, labelDate: event.target.value }))}
                  className={inputClass}
                />
              </label>
              <label className="space-y-1">
                <span className="block text-[11px] text-fg-muted">{t('batch.attentionDate')}</span>
                <input
                  type="date"
                  value={dateForm.attentionDate}
                  onChange={(event) => setDateForm((prev) => ({ ...prev, attentionDate: event.target.value }))}
                  className={inputClass}
                />
              </label>
              <label className="space-y-1">
                <span className="block text-[11px] text-fg-muted">{t('batch.openedDate')}</span>
                <input
                  type="date"
                  value={dateForm.openedDate}
                  onChange={(event) => setDateForm((prev) => ({ ...prev, openedDate: event.target.value }))}
                  className={inputClass}
                />
              </label>
            </div>
            <div className="flex items-center gap-2">
              <Button size="sm" variant="primary" onClick={() => saveDates(view)}>
                {t('common.save')}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() =>
                  setOpenState(view, undefined)
                }
              >
                {t('batch.unopen')}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setCardAction({ key: '', action: null })}>
                {t('common.cancel')}
              </Button>
            </div>
          </div>
        )}

        {active === 'detail' && (
          <div className="mt-2 space-y-1 rounded-card border border-border-strong bg-surface p-2">
            <p className="text-[11px] text-fg-subtle">
              {t('batch.conserved', {
                intake: formatAmount(view.intake),
                adjusted: formatAmount(view.adjusted),
                consumed: formatAmount(view.consumed),
                discarded: formatAmount(view.discarded),
                remaining: formatAmount(Math.max(view.remaining, 0)),
              })}
            </p>
            <ul className="space-y-0.5">
              {movements
                .filter((m) => m.batchId === view.id)
                .map((movement) => (
                  <li key={movement.id} className="flex items-center gap-2 text-xs text-fg-muted">
                    <span className="tabular-nums">{movement.date}</span>
                    <span className="min-w-0 flex-1 truncate">
                      {t(`movement.${movement.kind}`)} {formatAmount(Math.abs(movement.amount))} {t(`unit.${view.unit}`)}
                    </span>
                  </li>
                ))}
              {view.movementCount === 0 && <li className="text-xs text-fg-subtle">{t('batch.noMovements')}</li>}
            </ul>
          </div>
        )}
      </li>
    )
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.3fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Plus />} title={t('add.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
          <input
            value={draft.name}
            onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
            placeholder={t('add.namePlaceholder')}
            className={inputClass}
            aria-label={t('add.namePlaceholder')}
          />
          <div className="grid grid-cols-2 gap-2">
            <label className="space-y-1">
              <span className="block text-[11px] text-fg-muted">{t('add.location')}</span>
              <select
                value={draft.location}
                onChange={(event) => setDraft((prev) => ({ ...prev, location: event.target.value }))}
                className={inputClass}
              >
                {settings.locations.map((location) => (
                  <option key={location.id} value={location.id}>
                    {locationName(location.id)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="block text-[11px] text-fg-muted">{t('add.intake')}</span>
              <div className="flex gap-2">
                <NumberInput
                  value={draft.intake}
                  onValueChange={(intake) => setDraft((prev) => ({ ...prev, intake }))}
                  min={0}
                  className="min-w-0 flex-1"
                  aria-label={t('add.intake')}
                />
                <select
                  value={draft.unit}
                  onChange={(event) => setDraft((prev) => ({ ...prev, unit: event.target.value as UnitId }))}
                  className="h-9 rounded-control border border-border bg-surface px-2 text-sm"
                  aria-label={t('batch.unit')}
                >
                  {UNITS.map((unit) => (
                    <option key={unit} value={unit}>
                      {t(`unit.${unit}`)}
                    </option>
                  ))}
                </select>
              </div>
            </label>
          </div>
          <div className="grid grid-cols-2 gap-2">
            <label className="space-y-1">
              <span className="block text-[11px] text-fg-muted">{t('add.purchasedDate')}</span>
              <input
                type="date"
                value={draft.purchasedDate}
                onChange={(event) => setDraft((prev) => ({ ...prev, purchasedDate: event.target.value }))}
                className={inputClass}
              />
            </label>
            <label className="space-y-1">
              <span className="block text-[11px] text-fg-muted">{t('add.labelDate')}</span>
              <input
                type="date"
                value={draft.labelDate}
                onChange={(event) => setDraft((prev) => ({ ...prev, labelDate: event.target.value }))}
                className={inputClass}
              />
            </label>
          </div>
          <Button block variant="primary" onClick={addBatch} disabled={busy}>
            <Plus />
            {t('add.submit')}
          </Button>
          <p className="text-[11px] text-fg-subtle">{t('add.hint')}</p>
        </Panel>

        <Panel icon={<Snowflake />} title={t('filters.title')} className="min-h-0 flex-1" bodyClassName="space-y-3 overflow-auto p-3">
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('filters.location')}</p>
            <div className="flex flex-wrap gap-1.5">
              <Button
                size="sm"
                variant={locationFilter === 'all' ? 'primary' : 'outline'}
                onClick={() => setLocationFilter('all')}
              >
                {t('filters.all')}
              </Button>
              {settings.locations.map((location: Location) => (
                <Button
                  key={location.id}
                  size="sm"
                  variant={locationFilter === location.id ? 'primary' : 'outline'}
                  onClick={() => setLocationFilter(location.id)}
                >
                  {locationName(location.id)}
                  {['fridge', 'freezer', 'pantry'].includes(location.id) && <Snowflake className="size-3" />}
                </Button>
              ))}
            </div>
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('filters.opened')}</p>
            <Button size="sm" variant={openedOnly ? 'primary' : 'outline'} onClick={() => setOpenedOnly((prev) => !prev)}>
              {t('filters.openedOnly')}
            </Button>
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('filters.soonDays')}</p>
            <NumberInput
              value={settings.soonDays}
              onValueChange={(soonDays) => setSettings({ ...settings, soonDays })}
              min={0}
              max={MAX_SOON_DAYS}
              className="w-28"
              aria-label={t('filters.soonDays')}
            />
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('filters.locations')}</p>
            <div className="flex items-center gap-2">
              <input
                value={newLocation}
                onChange={(event) => setNewLocation(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') addLocation()
                }}
                placeholder={t('filters.locationPlaceholder')}
                className={cn(inputClass, 'min-w-0 flex-1')}
                aria-label={t('filters.locationPlaceholder')}
              />
              <Button size="sm" variant="outline" onClick={addLocation}>
                <Plus />
                {t('members.add')}
              </Button>
            </div>
            <ul className="flex flex-wrap gap-1.5">
              {settings.locations.map((location) => (
                <li key={location.id} className="flex items-center gap-1 rounded-full border border-border py-0.5 pl-2.5 pr-0.5 text-xs">
                  {locationName(location.id)}
                  {batches.some((b) => b.location === location.id) ? (
                    <span className="mx-1 text-fg-subtle">{t('filters.inUse')}</span>
                  ) : (
                    <Tooltip content={t('members.remove')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('members.remove')} onClick={() => removeLocation(location.id)}>
                        <Trash2 />
                      </Button>
                    </Tooltip>
                  )}
                </li>
              ))}
            </ul>
          </div>
        </Panel>
      </div>

      <Panel
        icon={<Snowflake />}
        title={t('board.title')}
        className="min-h-0"
        bodyClassName="min-h-0 space-y-4 overflow-auto p-3"
        actions={
          <div className="flex items-center gap-1">
            <Tooltip content={t('table.copy')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('table.copy')} onClick={() => void copyList()}>
                <ClipboardCopy />
              </Button>
            </Tooltip>
            <Tooltip content={t('table.saveTxt')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('table.saveTxt')} onClick={() => void saveList()}>
                <Download />
              </Button>
            </Tooltip>
            <Tooltip content={t('table.saveJson')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('table.saveJson')} onClick={() => void saveBackup()}>
                <Download />
              </Button>
            </Tooltip>
          </div>
        }
        footer={<span className="text-xs text-fg-subtle">{t('board.hint')}</span>}
      >
        {!review ? (
          <div className="flex items-center justify-center py-10">
            <Spinner className="size-6 text-fg-subtle" />
          </div>
        ) : (
          groups.map((group) => (
            <section key={group.key} className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">
                {t(`group.${group.key}`, { days: settings.soonDays })}
                <span className="ml-1 text-fg-subtle">{group.list.length}</span>
              </p>
              {group.list.length === 0 ? (
                <p className="px-1 text-xs text-fg-subtle">{t('group.empty')}</p>
              ) : (
                <ul className="space-y-1.5">
                  {group.list.map((view) => renderCard(view))}
                </ul>
              )}
            </section>
          ))
        )}
        {review && review.batches.length === 0 && (
          <Empty icon={<Snowflake />} title={t('board.empty')} description={t('board.emptyHint')} />
        )}
      </Panel>
    </div>
  )
}
