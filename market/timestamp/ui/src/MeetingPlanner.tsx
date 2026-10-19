import { useMemo, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Select, Spinner, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { CalendarRange, Copy, Plus, Trash2, TriangleAlert, Users } from 'lucide-react'
import type { PlanArgs, PlanOutput, PlannerParticipant, Slot } from './types'

interface PlannerState {
  participants: PlannerParticipant[]
  baseTimezone: string
  startDate: string
  durationMinutes: number
  searchDays: number
  groups: { name: string; participants: PlannerParticipant[] }[]
}

function localToday(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${month}-${day}`
}

const initialState = (): PlannerState => ({
  participants: [],
  baseTimezone: 'local',
  startDate: localToday(),
  durationMinutes: 60,
  searchDays: 14,
  groups: [],
})

function hhmm(minute: number): string {
  return minute >= 1440 ? '24:00' : `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`
}

function newId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

const newSlot = (): Slot => ({ day: 0, start: '09:00', end: '18:00', choices: [] })

const DAY_OPTIONS = [0, 1, 2, 3, 4, 5, 6]

export function MeetingPlanner({ zones }: { zones: string[] }) {
  const { t, errorMessage, manifest } = usePlugin()
  const [state, setState] = usePluginState<PlannerState>('planner', initialState())
  const [result, setResult] = useState<PlanOutput | null>(null)
  const [failure, setFailure] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const [groupName, setGroupName] = useState('')

  const timezoneOptions = useMemo(
    () => [{ value: 'local', label: t('planner.tzLocal') }, ...zones.map((zone) => ({ value: zone, label: zone }))],
    [zones, t],
  )

  const patchParticipant = (id: string, fields: Partial<PlannerParticipant>) => {
    setState({ ...state, participants: state.participants.map((p) => (p.id === id ? { ...p, ...fields } : p)) })
  }

  const patchSlot = (participantId: string, index: number, fields: Partial<Slot>) => {
    setState({
      ...state,
      participants: state.participants.map((p) =>
        p.id === participantId
          ? { ...p, slots: p.slots.map((slot, i) => (i === index ? { ...slot, ...fields } : slot)) }
          : p,
      ),
    })
  }

  const addParticipant = () => {
    setState({
      ...state,
      participants: [...state.participants, { id: newId(), label: '', timezone: 'local', slots: [newSlot()] }],
    })
  }

  const removeParticipant = (id: string) => {
    setState({ ...state, participants: state.participants.filter((p) => p.id !== id) })
  }

  const plan = async () => {
    const args: PlanArgs = {
      participants: state.participants.map((p) => ({ label: p.label, timezone: p.timezone, slots: p.slots })),
      baseTimezone: state.baseTimezone,
      startDate: state.startDate,
      durationMinutes: state.durationMinutes,
      searchDays: state.searchDays,
    }
    setPending(true)
    try {
      const out = await callPlugin<PlanOutput>(manifest.id, 'plan', args)
      setResult(out)
      setFailure(null)
    } catch (reason) {
      setFailure(errorMessage(reason))
      setResult(null)
    } finally {
      setPending(false)
    }
  }

  const resolveAmbiguity = (ambiguity: PlanOutput['ambiguities'][number], offsetSeconds: number) => {
    setState({
      ...state,
      participants: state.participants.map((p, pi) =>
        pi === ambiguity.participantIndex
          ? {
              ...p,
              slots: p.slots.map((slot, si) =>
                si === ambiguity.slotIndex
                  ? {
                      ...slot,
                      choices: [...slot.choices.filter((c) => c.date !== ambiguity.date), { date: ambiguity.date, offsetSeconds }],
                    }
                  : slot,
              ),
            }
          : p,
      ),
    })
  }

  const saveGroup = () => {
    const name = groupName.trim()
    if (!name || state.participants.length === 0) return
    setState({
      ...state,
      groups: [...state.groups.filter((group) => group.name !== name), { name, participants: state.participants }],
    })
    setGroupName('')
    toast.success(t('common:saved'))
  }

  const loadGroup = (name: string) => {
    const group = state.groups.find((item) => item.name === name)
    if (group) setState({ ...state, participants: group.participants })
  }

  const removeGroup = (name: string) => {
    setState({ ...state, groups: state.groups.filter((group) => group.name !== name) })
  }

  const copyText = async () => {
    if (!result) return
    await host.clipboard.writeText(result.text)
    toast.success(t('common:copied'))
  }

  const totalWindows = result?.days.reduce((sum, day) => sum + day.windows.length, 0) ?? 0

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(380px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        <Panel
          icon={<Users />}
          title={t('planner.participants')}
          bodyClassName="space-y-3 p-3"
          actions={
            <Button size="sm" variant="ghost" onClick={addParticipant}>
              <Plus />
              {t('planner.addParticipant')}
            </Button>
          }
        >
          {state.participants.length === 0 && <p className="text-xs text-fg-subtle">{t('planner.emptyParticipants')}</p>}
          {state.participants.map((participant, index) => (
            <div key={participant.id} className="space-y-2 rounded-lg border border-border-subtle p-2">
              <div className="flex items-center gap-2">
                <Input
                  value={participant.label}
                  onChange={(event) => patchParticipant(participant.id, { label: event.target.value })}
                  placeholder={t('planner.participantName')}
                  size="sm"
                  aria-label={t('planner.participantName')}
                  wrapperClassName="w-28"
                />
                <Select
                  value={participant.timezone}
                  onValueChange={(timezone) => patchParticipant(participant.id, { timezone })}
                  size="sm"
                  aria-label={t('planner.timezone')}
                  options={timezoneOptions}
                  className="flex-1"
                />
                <Tooltip content={t('planner.removeParticipant')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('planner.removeParticipant')} onClick={() => removeParticipant(participant.id)}>
                    <Trash2 />
                  </Button>
                </Tooltip>
              </div>
              {participant.slots.map((slot, slotIndex) => (
                <div key={slotIndex} className="flex items-center gap-2">
                  <Badge variant="neutral">#{index + 1}</Badge>
                  <Select
                    value={String(slot.day)}
                    onValueChange={(day) => patchSlot(participant.id, slotIndex, { day: Number(day) })}
                    size="sm"
                    aria-label={t('planner.weekday')}
                    options={DAY_OPTIONS.map((day) => ({ value: String(day), label: t(`weekdays.${day + 1}`) }))}
                    className="w-28"
                  />
                  <Input
                    type="time"
                    value={slot.start}
                    onChange={(event) => patchSlot(participant.id, slotIndex, { start: event.target.value })}
                    size="sm"
                    aria-label={t('planner.slotStart')}
                    wrapperClassName="w-28"
                  />
                  <span className="text-xs text-fg-subtle">{t('planner.rangeSeparator')}</span>
                  <Input
                    type="time"
                    value={slot.end}
                    onChange={(event) => patchSlot(participant.id, slotIndex, { end: event.target.value })}
                    size="sm"
                    aria-label={t('planner.slotEnd')}
                    wrapperClassName="w-28"
                  />
                  <Tooltip content={t('planner.removeSlot')}>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label={t('planner.removeSlot')}
                      onClick={() => patchParticipant(participant.id, { slots: participant.slots.filter((_, i) => i !== slotIndex) })}
                    >
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </div>
              ))}
              <Button size="sm" variant="ghost" onClick={() => patchParticipant(participant.id, { slots: [...participant.slots, newSlot()] })}>
                <Plus />
                {t('planner.addSlot')}
              </Button>
            </div>
          ))}
        </Panel>

        <Panel icon={<CalendarRange />} title={t('planner.settings')} bodyClassName="space-y-2 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <Input
              type="date"
              value={state.startDate}
              onChange={(event) => setState({ ...state, startDate: event.target.value })}
              size="sm"
              aria-label={t('planner.startDate')}
              wrapperClassName="w-40"
            />
            <Select
              value={state.baseTimezone}
              onValueChange={(baseTimezone) => setState({ ...state, baseTimezone })}
              size="sm"
              aria-label={t('planner.baseTimezone')}
              options={timezoneOptions}
              className="w-48"
            />
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Input
              value={state.durationMinutes}
              onChange={(event) => setState({ ...state, durationMinutes: Number(event.target.value.replace(/[^\d]/g, '')) || 0 })}
              size="sm"
              aria-label={t('planner.duration')}
              wrapperClassName="w-24"
              inputMode="numeric"
            />
            <span className="text-xs text-fg-muted">{t('planner.durationUnit')}</span>
            <Input
              value={state.searchDays}
              onChange={(event) => setState({ ...state, searchDays: Number(event.target.value.replace(/[^\d]/g, '')) || 0 })}
              size="sm"
              aria-label={t('planner.searchDays')}
              wrapperClassName="w-20"
              inputMode="numeric"
            />
            <span className="text-xs text-fg-muted">{t('planner.searchDaysUnit')}</span>
          </div>
          <Button onClick={() => void plan()} disabled={state.participants.length === 0 || pending}>
            {pending && <Spinner className="size-3.5" />}
            {t('planner.plan')}
          </Button>
        </Panel>

        <Panel icon={<Users />} title={t('planner.groups')} bodyClassName="space-y-2 p-3">
          <div className="flex items-center gap-2">
            <Input
              value={groupName}
              onChange={(event) => setGroupName(event.target.value)}
              placeholder={t('planner.groupName')}
              size="sm"
              aria-label={t('planner.groupName')}
              wrapperClassName="w-40"
            />
            <Button size="sm" variant="ghost" onClick={saveGroup} disabled={!groupName.trim() || state.participants.length === 0}>
              {t('planner.saveGroup')}
            </Button>
          </div>
          {state.groups.length === 0 && <p className="text-xs text-fg-subtle">{t('planner.emptyGroups')}</p>}
          {state.groups.map((group) => (
            <div key={group.name} className="flex items-center gap-2">
              <Badge variant="neutral">{group.name}</Badge>
              <Button size="sm" variant="ghost" onClick={() => loadGroup(group.name)}>
                {t('planner.loadGroup')}
              </Button>
              <Tooltip content={t('planner.removeGroup')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('planner.removeGroup')} onClick={() => removeGroup(group.name)}>
                  <Trash2 />
                </Button>
              </Tooltip>
            </div>
          ))}
        </Panel>
      </div>

      <Panel
        icon={<CalendarRange />}
        title={t('planner.results')}
        bodyClassName="space-y-3 p-3"
        actions={
          result && (
            <Tooltip content={t('planner.copy')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('planner.copy')} onClick={() => void copyText()}>
                <Copy />
              </Button>
            </Tooltip>
          )
        }
      >
        <div className="min-h-0 flex-1 space-y-3 overflow-y-auto">
          {failure && <p className="text-xs text-fg-danger">{failure}</p>}
          {result && result.gaps.length > 0 && (
            <div className="space-y-1 rounded-lg border border-amber-500/40 bg-amber-500/10 p-2">
              {result.gaps.map((gap, index) => (
                <p key={index} className="flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
                  <TriangleAlert className="size-3.5 shrink-0" />
                  {t('planner.gapNotice', { label: gap.label, date: gap.date, time: gap.time.slice(0, 5) })}
                </p>
              ))}
            </div>
          )}
          {result &&
            result.ambiguities.length > 0 && (
              <div className="space-y-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-2">
                {result.ambiguities.map((ambiguity, index) => (
                  <div key={index} className="space-y-1 text-xs text-amber-600 dark:text-amber-400">
                    <p className="flex items-center gap-1">
                      <TriangleAlert className="size-3.5 shrink-0" />
                      {t('planner.ambiguityNotice', {
                        label: ambiguity.label,
                        date: ambiguity.date,
                        time: ambiguity.time.slice(0, 5),
                        earlier: ambiguity.earlierOffset,
                        later: ambiguity.laterOffset,
                      })}
                    </p>
                    <div className="flex items-center gap-2">
                      <Button size="sm" variant="ghost" onClick={() => resolveAmbiguity(ambiguity, ambiguity.earlierOffsetSeconds)}>
                        {t('planner.useEarlier', { offset: ambiguity.earlierOffset })}
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => resolveAmbiguity(ambiguity, ambiguity.laterOffsetSeconds)}>
                        {t('planner.useLater', { offset: ambiguity.laterOffset })}
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          {!result && !failure && <Empty title={t('planner.emptyResults')} />}
          {result && totalWindows === 0 && <Empty title={t('planner.noWindows')} />}
          {result &&
            result.days
              .filter((day) => day.windows.length > 0)
              .map((day) => (
                <div key={day.date} className="space-y-2">
                  <p className="text-sm font-medium">
                    {day.date} · {t(`weekdays.${day.weekday + 1}`)}
                  </p>
                  {day.windows.map((window, index) => (
                    <div key={index} className="space-y-1 rounded-lg border border-border-subtle p-2">
                      <div className="flex items-center justify-between gap-2">
                        <p className="font-mono text-sm">{t('planner.windowLine', { start: hhmm(window.startMinute), end: hhmm(window.endMinute) })}</p>
                        <Badge variant="neutral">{t('planner.durationValue', { count: window.durationMinutes })}</Badge>
                      </div>
                      {window.rows.map((row, rowIndex) => (
                        <p key={rowIndex} className="text-xs text-fg-muted">
                          {t('planner.rowLine', {
                            label: row.label,
                            timezone: row.timezone,
                            date: row.date,
                            weekday: t(`weekdays.${row.weekday + 1}`),
                            start: hhmm(row.startMinute),
                            end: hhmm(row.endMinute),
                          })}
                          {row.crossDay && t('planner.crossDaySuffix', { date: row.endDate })}
                        </p>
                      ))}
                      <p className="text-xs text-fg-subtle">{t('planner.utcLine', { start: window.utcStart, end: window.utcEnd })}</p>
                    </div>
                  ))}
                </div>
              ))}
        </div>
      </Panel>
    </div>
  )
}
