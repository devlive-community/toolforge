import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Empty,
  NumberInput,
  Panel,
  SegmentedControl,
  Tooltip,
  cn,
  toast,
} from '@toolforge/ui'
import { host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import {
  Check,
  ClipboardCopy,
  Download,
  Pause,
  Pencil,
  Play,
  Plus,
  RotateCcw,
  SkipForward,
  Trash2,
  UserPlus,
  Users,
} from 'lucide-react'
import {
  addDaysText,
  mondayOfWeek,
  todayText,
  weekdayOf,
  WEEKDAY_LABEL_KEYS,
  type Chore,
  type Epoch,
  type Member,
  type Mode,
  type Occurrence,
  type OccurrenceRecord,
  type PlanOut,
  type Rule,
  MAX_CHORES,
  MAX_INTERVAL_DAYS,
  MAX_MEMBERS,
  OVERDUE_LOOKBACK_DAYS,
} from './types'

type RuleKind = 'daily' | 'weekly' | 'interval'
type ModeKind = 'fixed' | 'rotation'

interface ChoreDraft {
  name: string
  ruleKind: RuleKind
  weekdays: number[]
  intervalDays: number
  modeKind: ModeKind
  fixedMemberId: string
  rotationIds: string[]
  startDate: string
}

const EMPTY_DRAFT: ChoreDraft = {
  name: '',
  ruleKind: 'daily',
  weekdays: [1, 3, 5],
  intervalDays: 3,
  modeKind: 'rotation',
  fixedMemberId: '',
  rotationIds: [],
  startDate: todayText(),
}

function draftRule(draft: ChoreDraft): Rule {
  if (draft.ruleKind === 'weekly') return { kind: 'weekly', weekdays: [...draft.weekdays].sort() }
  if (draft.ruleKind === 'interval') return { kind: 'interval', days: draft.intervalDays }
  return { kind: 'daily' }
}

function draftMode(draft: ChoreDraft): Mode {
  if (draft.modeKind === 'fixed') return { type: 'fixed', memberId: draft.fixedMemberId }
  return { type: 'rotation', memberIds: [...draft.rotationIds] }
}

function epochFromDraft(draft: ChoreDraft, from: string): Epoch {
  return { from, rule: draftRule(draft), mode: draftMode(draft) }
}

export function Chores() {
  const { t, call, errorMessage } = usePlugin()
  const [members, setMembers] = usePluginState<Member[]>('members', [])
  const [chores, setChores] = usePluginState<Chore[]>('chores', [])
  const [records, setRecords] = usePluginState<Record<string, OccurrenceRecord>>('records', {})

  const [view, setView] = useState<'today' | 'week'>('today')
  const [planToday, setPlanToday] = useState<PlanOut | null>(null)
  const [planWeek, setPlanWeek] = useState<PlanOut | null>(null)
  const [memberInput, setMemberInput] = useState('')
  const [editorId, setEditorId] = useState<string | null>(null)
  const [draft, setDraft] = useState<ChoreDraft>({ ...EMPTY_DRAFT, startDate: todayText() })
  const planSeq = useRef(0)

  const today = todayText()
  const monday = mondayOfWeek()
  const sunday = addDaysText(monday, 6)

  const nameOf = useCallback(
    (id: string) => members.find((m) => m.id === id)?.name ?? t('common.removed'),
    [members, t],
  )

  // 排期 / 状态 / 统计全部由后端计算；今日看板回看逾期，本周看板周一到周日
  useEffect(() => {
    const seq = ++planSeq.current
    const timer = setTimeout(() => {
      void (async () => {
        try {
          const [todayOut, weekOut] = await Promise.all([
            call<PlanOut>('plan', {
              members,
              chores,
              records,
              today,
              fromDate: addDaysText(today, -OVERDUE_LOOKBACK_DAYS),
              toDate: today,
            }),
            call<PlanOut>('plan', { members, chores, records, today, fromDate: monday, toDate: sunday }),
          ])
          if (seq !== planSeq.current) return
          setPlanToday(todayOut)
          setPlanWeek(weekOut)
        } catch (error) {
          if (seq === planSeq.current) toast.error(errorMessage(error))
        }
      })()
    }, 60)
    return () => clearTimeout(timer)
  }, [call, chores, errorMessage, members, monday, records, sunday, today])

  const activePlan = view === 'today' ? planToday : planWeek

  const addMember = () => {
    const name = memberInput.trim()
    if (!name) return
    if (members.length >= MAX_MEMBERS) {
      toast.error(t('errors.chore.too_many_members', { limit: MAX_MEMBERS }))
      return
    }
    setMembers((prev) => [...prev, { id: crypto.randomUUID(), name }])
    setMemberInput('')
  }

  const removeMember = (id: string) => {
    setMembers((prev) => prev.filter((m) => m.id !== id))
    setRecords((prev) => {
      // 历史记录里的执行者快照保留；仅清理以该成员为临时换人的覆盖
      const next: Record<string, OccurrenceRecord> = {}
      for (const [key, record] of Object.entries(prev)) {
        if (record.assignee === id) {
          next[key] = { ...record, assignee: undefined }
        } else {
          next[key] = record
        }
      }
      return next
    })
  }

  const openEditor = (chore: Chore | null) => {
    if (!chore) {
      setEditorId('new')
      setDraft({ ...EMPTY_DRAFT, startDate: today, fixedMemberId: members[0]?.id ?? '', rotationIds: members.slice(0, 1).map((m) => m.id) })
      return
    }
    const last = chore.epochs[chore.epochs.length - 1]
    setEditorId(chore.id)
    setDraft({
      name: chore.name,
      ruleKind: last.rule.kind,
      weekdays: last.rule.kind === 'weekly' ? [...last.rule.weekdays] : [1, 3, 5],
      intervalDays: last.rule.kind === 'interval' ? last.rule.days : 3,
      modeKind: last.mode.type,
      fixedMemberId: last.mode.type === 'fixed' ? last.mode.memberId : members[0]?.id ?? '',
      rotationIds: last.mode.type === 'rotation' ? [...last.mode.memberIds] : members.slice(0, 1).map((m) => m.id),
      startDate: last.from,
    })
  }

  const closeEditor = () => setEditorId(null)

  const saveChore = () => {
    const name = draft.name.trim()
    if (!name) return
    if (draft.modeKind === 'fixed' && !draft.fixedMemberId) {
      toast.error(t('errors.chore.member_required'))
      return
    }
    if (draft.modeKind === 'rotation' && draft.rotationIds.length === 0) {
      toast.error(t('errors.chore.rotation_required'))
      return
    }
    if (draft.ruleKind === 'weekly' && draft.weekdays.length === 0) {
      toast.error(t('errors.chore.weekday_required'))
      return
    }
    if (editorId === 'new') {
      if (chores.length >= MAX_CHORES) {
        toast.error(t('errors.chore.too_many_chores', { limit: MAX_CHORES }))
        return
      }
      const chore: Chore = {
        id: crypto.randomUUID(),
        name,
        epochs: [epochFromDraft(draft, draft.startDate || today)],
      }
      setChores((prev) => [...prev, chore])
    } else if (editorId) {
      // 修改只从今天生效：追加纪元；与最后一个纪元同日则直接替换
      setChores((prev) =>
        prev.map((chore) => {
          if (chore.id !== editorId) return chore
          const epochs = [...chore.epochs]
          const nextEpoch = epochFromDraft(draft, today > epochs[epochs.length - 1].from ? today : epochs[epochs.length - 1].from)
          if (epochs[epochs.length - 1].from === nextEpoch.from) {
            epochs[epochs.length - 1] = nextEpoch
          } else {
            epochs.push(nextEpoch)
          }
          return { ...chore, name, epochs }
        }),
      )
    }
    closeEditor()
  }

  const togglePause = (chore: Chore) => {
    setChores((prev) =>
      prev.map((item) => {
        if (item.id !== chore.id) return item
        if (item.pausedFrom) {
          // 恢复：从今天起按原规则重新排期，停用期间不补生成
          const last = item.epochs[item.epochs.length - 1]
          return {
            ...item,
            pausedFrom: undefined,
            epochs: [...item.epochs, { from: today > last.from ? today : last.from, rule: last.rule, mode: last.mode }],
          }
        }
        return { ...item, pausedFrom: today }
      }),
    )
  }

  const deleteChore = (id: string) => {
    setChores((prev) => prev.filter((chore) => chore.id !== id))
    setRecords((prev) => {
      const next: Record<string, OccurrenceRecord> = {}
      for (const [key, record] of Object.entries(prev)) {
        if (!key.startsWith(`${id}|`)) next[key] = record
      }
      return next
    })
  }

  /** 完成一次：重复点击幂等，逾期补做保留计划日期并记录实际日期 */
  const complete = (occurrence: Occurrence) => {
    if (occurrence.status === 'done') return
    setRecords((prev) => ({
      ...prev,
      [occurrence.key]: {
        ...prev[occurrence.key],
        status: 'done',
        executor: prev[occurrence.key]?.executor ?? occurrence.assigneeId,
        actualDate: today,
      },
    }))
  }

  const skip = (occurrence: Occurrence) => {
    if (occurrence.status === 'skipped') return
    setRecords((prev) => ({
      ...prev,
      [occurrence.key]: { ...prev[occurrence.key], status: 'skipped' },
    }))
  }

  const undo = (occurrence: Occurrence) => {
    setRecords((prev) => {
      const next = { ...prev }
      const record = next[occurrence.key]
      if (record?.assignee) {
        next[occurrence.key] = { assignee: record.assignee }
      } else {
        delete next[occurrence.key]
      }
      return next
    })
  }

  const reassign = (occurrence: Occurrence, memberId: string) => {
    setRecords((prev) => ({
      ...prev,
      [occurrence.key]: { ...prev[occurrence.key], assignee: memberId },
    }))
  }

  const statusButton = (occurrence: Occurrence) => {
    if (occurrence.status === 'pending') {
      return (
        <div className="flex items-center gap-1">
          <Tooltip content={t('board.done')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('board.done')} onClick={() => complete(occurrence)}>
              <Check />
            </Button>
          </Tooltip>
          <Tooltip content={t('board.skip')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('board.skip')} onClick={() => skip(occurrence)}>
              <SkipForward />
            </Button>
          </Tooltip>
        </div>
      )
    }
    return (
      <Tooltip content={t('board.undo')}>
        <Button size="icon-sm" variant="ghost" aria-label={t('board.undo')} onClick={() => undo(occurrence)}>
          <RotateCcw />
        </Button>
      </Tooltip>
    )
  }

  /** 复制 / 保存轮值表（本周） */
  const tableText = useMemo(() => {
    if (!planWeek) return ''
    const byDate = new Map<string, Occurrence[]>()
    for (const occurrence of planWeek.occurrences) {
      const list = byDate.get(occurrence.date) ?? []
      list.push(occurrence)
      byDate.set(occurrence.date, list)
    }
    const lines: string[] = [t('table.title', { from: monday, to: sunday })]
    for (let i = 0; i < 7; i++) {
      const date = addDaysText(monday, i)
      const list = byDate.get(date) ?? []
      const body =
        list.length === 0
          ? t('table.rest')
          : list
              .map((o) => {
                const who = o.assigneeName ?? t('common.removed')
                const mark = o.status === 'done' ? t('table.done') : o.status === 'skipped' ? t('table.skipped') : ''
                return `${o.choreName}→${who}${mark}`
              })
              .join('；')
      lines.push(`${date} ${t(`weekday.${WEEKDAY_LABEL_KEYS[weekdayOf(date)]}`)}：${body}`)
    }
    return lines.join('\n')
  }, [monday, planWeek, sunday, t])

  const copyTable = async () => {
    try {
      await host.clipboard.writeText(tableText)
      toast.success(t('table.copied'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const saveTable = async () => {
    try {
      const path = await host.dialog.saveFile(`chores-${monday}.txt`, [{ name: 'TXT', extensions: ['txt'] }])
      if (!path) return
      await host.fs.writeText(path, tableText)
      toast.success(t('table.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const saveBackup = async () => {
    try {
      const path = await host.dialog.saveFile(`chores-backup-${today}.json`, [
        { name: 'JSON', extensions: ['json'] },
      ])
      if (!path) return
      await host.fs.writeText(path, JSON.stringify({ members, chores, records }, null, 2))
      toast.success(t('table.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const renderOccurrence = (occurrence: Occurrence) => (
    <li
      key={occurrence.key}
      className={cn(
        'flex items-center gap-2 rounded-control px-2 py-1.5 text-xs',
        occurrence.missed && 'bg-danger/5',
      )}
    >
      {occurrence.status === 'done' ? (
        <Badge variant="success">{t('board.doneBadge')}</Badge>
      ) : occurrence.status === 'skipped' ? (
        <Badge>{t('board.skipBadge')}</Badge>
      ) : occurrence.missed ? (
        <Badge variant="danger">{t('board.overdue')}</Badge>
      ) : (
        <Badge variant="info">{t('board.pendingBadge')}</Badge>
      )}
      <span className="min-w-0 flex-1 truncate">
        {occurrence.choreName}
        <span className="text-fg-subtle"> · {occurrence.assigneeName ?? t('common.removed')}</span>
        {occurrence.overridden && <span className="ml-1 text-fg-subtle">{t('board.tempSwap')}</span>}
        {occurrence.late && <span className="ml-1 text-fg-subtle">{t('board.lateBadge')}</span>}
      </span>
      {occurrence.status === 'pending' && (
        <select
          value=""
          onChange={(event) => {
            if (event.target.value) reassign(occurrence, event.target.value)
          }}
          className="h-6 rounded-control border border-border bg-surface px-1 text-xs text-fg-muted"
          aria-label={t('board.swap')}
        >
          <option value="">{t('board.swap')}</option>
          {members.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </select>
      )}
      {statusButton(occurrence)}
    </li>
  )

  const boardOccurrences = useMemo(() => activePlan?.occurrences ?? [], [activePlan])
  const todayList = view === 'today' ? boardOccurrences.filter((o) => o.date === today) : []
  const overdueList = view === 'today' ? boardOccurrences.filter((o) => o.date !== today) : []
  const weekGroups = useMemo(() => {
    if (view !== 'week') return []
    const groups: Array<{ date: string; list: Occurrence[] }> = []
    for (let i = 0; i < 7; i++) {
      const date = addDaysText(monday, i)
      groups.push({ date, list: boardOccurrences.filter((o) => o.date === date) })
    }
    return groups
  }, [boardOccurrences, monday, view])

  const stats = planWeek?.stats ?? []

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.3fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Users />} title={t('members.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          <div className="flex items-center gap-2">
            <input
              value={memberInput}
              onChange={(event) => setMemberInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') addMember()
              }}
              placeholder={t('members.placeholder')}
              className="h-9 min-w-0 flex-1 rounded-control border border-border bg-surface px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label={t('members.placeholder')}
            />
            <Button size="sm" variant="outline" onClick={addMember}>
              <UserPlus />
              {t('members.add')}
            </Button>
          </div>
          {members.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('members.empty')}</p>
          ) : (
            <ul className="flex flex-wrap gap-1.5">
              {members.map((m) => (
                <li key={m.id} className="flex items-center gap-1 rounded-full border border-border py-0.5 pl-2.5 pr-0.5 text-xs">
                  {m.name}
                  <Tooltip content={t('members.remove')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('members.remove')} onClick={() => removeMember(m.id)}>
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel
          icon={<Check />}
          title={t('chore.title')}
          className="min-h-0 flex-1"
          bodyClassName="space-y-2 overflow-auto p-3"
          actions={
            <Button size="sm" variant="outline" onClick={() => openEditor(null)}>
              <Plus />
              {t('chore.add')}
            </Button>
          }
        >
          {chores.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('chore.empty')}</p>
          ) : (
            <ul className="space-y-1.5">
              {chores.map((chore) => {
                const last = chore.epochs[chore.epochs.length - 1]
                return (
                  <li key={chore.id} className="rounded-card border border-border p-2">
                    <div className="flex items-center gap-1.5">
                      <span className="min-w-0 flex-1 truncate text-xs font-medium">
                        {chore.name}
                        {chore.pausedFrom && <Badge variant="danger">{t('chore.paused')}</Badge>}
                      </span>
                      <Tooltip content={t('chore.edit')}>
                        <Button size="icon-sm" variant="ghost" aria-label={t('chore.edit')} onClick={() => openEditor(chore)}>
                          <Pencil />
                        </Button>
                      </Tooltip>
                      <Tooltip content={chore.pausedFrom ? t('chore.resume') : t('chore.pause')}>
                        <Button
                          size="icon-sm"
                          variant="ghost"
                          aria-label={chore.pausedFrom ? t('chore.resume') : t('chore.pause')}
                          onClick={() => togglePause(chore)}
                        >
                          {chore.pausedFrom ? <Play /> : <Pause />}
                        </Button>
                      </Tooltip>
                      <Tooltip content={t('chore.delete')}>
                        <Button size="icon-sm" variant="ghost" aria-label={t('chore.delete')} onClick={() => deleteChore(chore.id)}>
                          <Trash2 />
                        </Button>
                      </Tooltip>
                    </div>
                    <p className="mt-1 text-[11px] text-fg-subtle">
                      {last.rule.kind === 'daily'
                        ? t('chore.rule.daily')
                        : last.rule.kind === 'interval'
                          ? t('chore.rule.interval', { days: last.rule.days })
                          : t('chore.rule.weekly', { days: last.rule.weekdays.length })}
                      {' · '}
                      {last.mode.type === 'fixed'
                        ? t('chore.fixedBy', { name: nameOf(last.mode.memberId) })
                        : t('chore.rotationOf', { count: last.mode.memberIds.length })}
                      {t('chore.since', { from: last.from })}
                    </p>
                  </li>
                )
              })}
            </ul>
          )}

          {editorId !== null && (
            <div className="space-y-3 rounded-card border border-border-strong bg-surface p-3">
              <input
                value={draft.name}
                onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
                placeholder={t('chore.namePlaceholder')}
                className="h-9 w-full rounded-control border border-border bg-surface px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label={t('chore.namePlaceholder')}
              />
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('chore.repeat')}</p>
                <SegmentedControl<RuleKind>
                  value={draft.ruleKind}
                  onValueChange={(ruleKind) => setDraft((prev) => ({ ...prev, ruleKind }))}
                  aria-label={t('chore.repeat')}
                  options={[
                    { value: 'daily', label: t('chore.rule.daily') },
                    { value: 'weekly', label: t('chore.rule.weeklyShort') },
                    { value: 'interval', label: t('chore.rule.intervalShort') },
                  ]}
                />
                {draft.ruleKind === 'weekly' && (
                  <div className="flex flex-wrap gap-1">
                    {WEEKDAY_LABEL_KEYS.map((key, index) => {
                      const active = draft.weekdays.includes(index)
                      return (
                        <button
                          key={key}
                          type="button"
                          onClick={() =>
                            setDraft((prev) => ({
                              ...prev,
                              weekdays: active
                                ? prev.weekdays.filter((d) => d !== index)
                                : [...prev.weekdays, index],
                            }))
                          }
                          className={cn(
                            'size-7 rounded-control border text-xs',
                            active ? 'border-primary bg-primary text-fg-on-primary' : 'border-border text-fg-muted',
                          )}
                        >
                          {t(`weekday.${key}`)}
                        </button>
                      )
                    })}
                  </div>
                )}
                {draft.ruleKind === 'interval' && (
                  <NumberInput
                    value={draft.intervalDays}
                    onValueChange={(intervalDays) => setDraft((prev) => ({ ...prev, intervalDays }))}
                    min={1}
                    max={MAX_INTERVAL_DAYS}
                    aria-label={t('chore.rule.intervalShort')}
                  />
                )}
              </div>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('chore.who')}</p>
                <SegmentedControl<ModeKind>
                  value={draft.modeKind}
                  onValueChange={(modeKind) => setDraft((prev) => ({ ...prev, modeKind }))}
                  aria-label={t('chore.who')}
                  options={[
                    { value: 'fixed', label: t('chore.modeFixed') },
                    { value: 'rotation', label: t('chore.modeRotation') },
                  ]}
                />
                {draft.modeKind === 'fixed' ? (
                  <select
                    value={draft.fixedMemberId}
                    onChange={(event) => setDraft((prev) => ({ ...prev, fixedMemberId: event.target.value }))}
                    className="h-9 w-full rounded-control border border-border bg-surface px-2 text-sm"
                    aria-label={t('chore.modeFixed')}
                  >
                    <option value="">{t('chore.pickMember')}</option>
                    {members.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.name}
                      </option>
                    ))}
                  </select>
                ) : (
                  <div className="space-y-1">
                    {members.length === 0 && <p className="text-xs text-fg-subtle">{t('members.empty')}</p>}
                    <div className="flex flex-wrap gap-1.5">
                      {members.map((m) => {
                        const order = draft.rotationIds.indexOf(m.id)
                        const active = order >= 0
                        return (
                          <button
                            key={m.id}
                            type="button"
                            onClick={() =>
                              setDraft((prev) => ({
                                ...prev,
                                rotationIds: active
                                  ? prev.rotationIds.filter((id) => id !== m.id)
                                  : [...prev.rotationIds, m.id],
                              }))
                            }
                            className={cn(
                              'rounded-full border px-2.5 py-0.5 text-xs',
                              active ? 'border-primary bg-primary text-fg-on-primary' : 'border-border text-fg-muted',
                            )}
                          >
                            {active && <span className="mr-0.5 tabular-nums">{order + 1}.</span>}
                            {m.name}
                          </button>
                        )
                      })}
                    </div>
                    <p className="text-[11px] text-fg-subtle">{t('chore.rotationHint')}</p>
                  </div>
                )}
              </div>
              {editorId === 'new' && (
                <div className="space-y-1.5">
                  <p className="text-xs font-medium text-fg-muted">{t('chore.firstDate')}</p>
                  <input
                    type="date"
                    value={draft.startDate}
                    onChange={(event) => setDraft((prev) => ({ ...prev, startDate: event.target.value }))}
                    className="h-9 w-full rounded-control border border-border bg-surface px-2 text-sm"
                    aria-label={t('chore.firstDate')}
                  />
                </div>
              )}
              {editorId !== 'new' && (
                <p className="text-[11px] text-fg-subtle">{t('chore.editHint', { from: today })}</p>
              )}
              <div className="grid grid-cols-2 gap-2">
                <Button variant="primary" onClick={saveChore}>
                  <Check />
                  {t('common.save')}
                </Button>
                <Button variant="outline" onClick={closeEditor}>
                  {t('common.cancel')}
                </Button>
              </div>
            </div>
          )}
        </Panel>
      </div>

      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<Check />}
          title={t('board.title')}
          className="min-h-0 flex-1"
          bodyClassName="min-h-0 space-y-3 overflow-auto p-3"
          extra={
            <SegmentedControl<'today' | 'week'>
              size="sm"
              value={view}
              onValueChange={setView}
              aria-label={t('board.view')}
              options={[
                { value: 'today', label: t('board.today') },
                { value: 'week', label: t('board.week') },
              ]}
            />
          }
          actions={
            <div className="flex items-center gap-1">
              <Tooltip content={t('table.copy')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('table.copy')} onClick={() => void copyTable()}>
                  <ClipboardCopy />
                </Button>
              </Tooltip>
              <Tooltip content={t('table.saveTxt')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('table.saveTxt')} onClick={() => void saveTable()}>
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
        >
          {!planToday || !planWeek ? (
            <p className="text-xs text-fg-subtle">{t('board.loading')}</p>
          ) : boardOccurrences.length === 0 ? (
            <Empty icon={<Check />} title={t('board.empty')} description={t('board.emptyHint')} />
          ) : view === 'today' ? (
            <>
              <section className="space-y-1">
                <p className="text-xs font-medium text-fg-muted">
                  {t('board.todayHeader', { date: today, weekday: t(`weekday.${WEEKDAY_LABEL_KEYS[weekdayOf(today)]}`) })}
                </p>
                {todayList.length === 0 ? (
                  <p className="px-2 text-xs text-fg-subtle">{t('board.nothingToday')}</p>
                ) : (
                  <ul className="space-y-1">{todayList.map(renderOccurrence)}</ul>
                )}
              </section>
              {overdueList.length > 0 && (
                <section className="space-y-1">
                  <p className="text-xs font-medium text-fg-muted">{t('board.overdueHeader')}</p>
                  <ul className="space-y-1">{overdueList.map(renderOccurrence)}</ul>
                </section>
              )}
            </>
          ) : (
            <div className="space-y-3">
              {weekGroups.map((group) => (
                <section key={group.date} className="space-y-1">
                  <p className="text-xs font-medium text-fg-muted">
                    {t('board.dayHeader', {
                      date: group.date,
                      weekday: t(`weekday.${WEEKDAY_LABEL_KEYS[weekdayOf(group.date)]}`),
                    })}
                  </p>
                  {group.list.length === 0 ? (
                    <p className="px-2 text-xs text-fg-subtle">{t('table.rest')}</p>
                  ) : (
                    <ul className="space-y-1">{group.list.map(renderOccurrence)}</ul>
                  )}
                </section>
              ))}
            </div>
          )}
        </Panel>

        <Panel icon={<Users />} title={t('stats.title')} className="shrink-0" bodyClassName="p-3">
          {stats.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('members.empty')}</p>
          ) : (
            <ul className="grid grid-cols-1 gap-1 sm:grid-cols-2">
              {stats.map((stat) => (
                <li key={stat.memberId} className="flex items-center gap-2 rounded-control px-2 py-1 text-xs">
                  <span className="min-w-0 flex-1 truncate">{stat.name}</span>
                  <Badge variant="success">{t('stats.done', { count: stat.done })}</Badge>
                  <Badge>{t('stats.skipped', { count: stat.skipped })}</Badge>
                  <Badge variant="info">{t('stats.pending', { count: stat.pending })}</Badge>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>
    </div>
  )
}
