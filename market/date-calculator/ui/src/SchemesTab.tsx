import { useState } from 'react'
import { Badge, Button, Checkbox, DropdownMenu, Empty, Input, Panel, SegmentedControl, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CalendarCog, ChevronDown, Download, FileUp, Plus, Trash2 } from 'lucide-react'
import { DateInput } from './DateInput'
import { toScheme, type Base, type Scheme, type StoredScheme } from './types'

const WEEKDAYS = [1, 2, 3, 4, 5, 6, 7]

const newId = () => crypto.randomUUID()

export function SchemesTab({ schemes, onChange, today }: { schemes: StoredScheme[]; onChange: (next: StoredScheme[]) => void; today: string }) {
  const { t, call, errorMessage } = usePlugin()
  const [selectedId, setSelectedId] = useState<string | null>(schemes[0]?.id ?? null)
  const [date, setDate] = useState(today)
  const [work, setWork] = useState(false)
  const [dayName, setDayName] = useState('')
  const selected = schemes.find((scheme) => scheme.id === selectedId) ?? null

  const update = (patch: Partial<Scheme>) => {
    if (!selected) return
    onChange(schemes.map((scheme) => (scheme.id === selected.id ? { ...scheme, ...patch } : scheme)))
  }

  const create = () => {
    const scheme: StoredScheme = { id: newId(), name: t('schemes.untitled', { n: schemes.length + 1 }), base: 'china', weekend: [6, 7], days: [] }
    onChange([...schemes, scheme])
    setSelectedId(scheme.id)
  }

  const remove = () => {
    if (!selected) return
    const next = schemes.filter((scheme) => scheme.id !== selected.id)
    onChange(next)
    setSelectedId(next[0]?.id ?? null)
  }

  const addDay = () => {
    if (!selected || !date.trim()) return
    const others = selected.days.filter((day) => day.date !== date)
    const days = [...others, { date, work, ...(dayName.trim() ? { name: dayName.trim() } : {}) }].sort((a, b) => a.date.localeCompare(b.date))
    update({ days })
    setDayName('')
  }

  const importScheme = async () => {
    try {
      const path = await host.dialog.openFile([{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      const scheme = await call<Scheme>('scheme_import', { text: await host.fs.readText(path) })
      const stored = { ...scheme, id: newId() }
      onChange([...schemes, stored])
      setSelectedId(stored.id)
      toast.success(t('schemes.imported', { name: scheme.name, count: scheme.days.length }))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const exportScheme = async (target: 'copy' | 'save') => {
    if (!selected) return
    try {
      const text = await call<string>('scheme_export', { scheme: toScheme(selected) })
      if (target === 'copy') {
        await host.clipboard.writeText(text)
        toast.success(t('common:copied'))
        return
      }
      const path = await host.dialog.saveFile(`${selected.name}.json`, [{ name: 'JSON', extensions: ['json'] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[240px_minmax(0,1fr)] gap-3">
      <Panel
        icon={<CalendarCog />}
        title={t('schemes.title')}
        actions={
          <>
            <Tooltip content={t('schemes.import')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('schemes.import')} onClick={() => void importScheme()}>
                <FileUp />
              </Button>
            </Tooltip>
            <Tooltip content={t('schemes.create')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('schemes.create')} onClick={create}>
                <Plus />
              </Button>
            </Tooltip>
          </>
        }
        bodyClassName="space-y-1 overflow-auto p-2"
      >
        {schemes.length === 0 ? (
          <p className="px-1 py-2 text-xs text-fg-subtle">{t('schemes.none')}</p>
        ) : (
          schemes.map((scheme) => (
            <button
              key={scheme.id}
              type="button"
              onClick={() => setSelectedId(scheme.id)}
              className={cn(
                'w-full rounded-control px-2.5 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                scheme.id === selectedId ? 'bg-primary-soft text-primary-fg' : 'hover:bg-hover',
              )}
            >
              <p className="truncate text-[13px] font-medium">{scheme.name}</p>
              <p className="text-[11px] text-fg-subtle">{t('schemes.summary', { base: t(`schemes.bases.${scheme.base}`), count: scheme.days.length })}</p>
            </button>
          ))
        )}
      </Panel>

      <Panel
        title={selected?.name ?? t('schemes.title')}
        actions={
          selected && (
            <>
              <DropdownMenu
                trigger={
                  <Button size="sm">
                    <Download />
                    {t('schemes.export')}
                    <ChevronDown />
                  </Button>
                }
                items={[
                  { key: 'copy', label: t('schemes.exportCopy'), onSelect: () => void exportScheme('copy') },
                  { key: 'save', label: t('schemes.exportSave'), onSelect: () => void exportScheme('save') },
                ]}
              />
              <Tooltip content={t('schemes.delete')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('schemes.delete')} onClick={remove}>
                  <Trash2 />
                </Button>
              </Tooltip>
            </>
          )
        }
        bodyClassName="space-y-4 overflow-auto p-3"
      >
        {!selected ? (
          <Empty icon={<CalendarCog />} title={t('schemes.empty')} description={t('schemes.emptyHint')} />
        ) : (
          <>
            <div className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
              <span className="text-xs text-fg-muted">{t('schemes.name')}</span>
              <Input size="sm" value={selected.name} onChange={(event) => update({ name: event.target.value })} aria-label={t('schemes.name')} />
              <span className="text-xs text-fg-muted">{t('schemes.base')}</span>
              <SegmentedControl<Base>
                size="sm"
                className="justify-self-start"
                value={selected.base}
                onValueChange={(base) => update({ base })}
                aria-label={t('schemes.base')}
                options={(['china', 'weekdays'] as Base[]).map((value) => ({ value, label: t(`schemes.bases.${value}`) }))}
              />
              <span className="text-xs text-fg-muted">{t('schemes.weekend')}</span>
              <div className="flex flex-wrap gap-3">
                {WEEKDAYS.map((day) => (
                  <Checkbox
                    key={day}
                    checked={selected.weekend.includes(day)}
                    onCheckedChange={(checked) =>
                      update({ weekend: checked ? [...selected.weekend, day].sort((a, b) => a - b) : selected.weekend.filter((d) => d !== day) })
                    }
                  >
                    <span className="text-[13px] text-fg">{t(`weekdaysShort.${day - 1}`)}</span>
                  </Checkbox>
                ))}
              </div>
            </div>
            <p className="text-[11px] leading-snug text-fg-subtle">{t('schemes.rules')}</p>

            <div className="space-y-2">
              <p className="text-xs font-medium text-fg-muted">{t('schemes.days', { count: selected.days.length })}</p>
              <div className="flex flex-wrap items-end gap-2 rounded-control border border-border p-2.5">
                <div className="w-48">
                  <DateInput label={t('schemes.date')} value={date} onChange={setDate} today={today} />
                </div>
                <SegmentedControl<'rest' | 'work'>
                  size="sm"
                  value={work ? 'work' : 'rest'}
                  onValueChange={(value) => setWork(value === 'work')}
                  aria-label={t('schemes.kind')}
                  options={[
                    { value: 'rest', label: t('schemes.restDay') },
                    { value: 'work', label: t('schemes.workDay') },
                  ]}
                />
                <Input size="sm" className="w-40" value={dayName} onChange={(event) => setDayName(event.target.value)} placeholder={t('schemes.dayName')} aria-label={t('schemes.dayName')} />
                <Button size="sm" variant="primary" onClick={addDay} disabled={!date.trim()}>
                  <Plus />
                  {t('schemes.addDay')}
                </Button>
              </div>
              {selected.days.length === 0 ? (
                <p className="text-xs text-fg-subtle">{t('schemes.noDays')}</p>
              ) : (
                <ul className="divide-y divide-border rounded-control border border-border">
                  {selected.days.map((day) => (
                    <li key={day.date} className="flex items-center gap-3 px-3 py-1.5 text-[13px]">
                      <span className="font-mono text-fg tabular-nums">{day.date}</span>
                      <Badge variant={day.work ? 'neutral' : 'success'}>{t(day.work ? 'schemes.workDay' : 'schemes.restDay')}</Badge>
                      <span className="min-w-0 flex-1 truncate text-fg-muted">{day.name}</span>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        aria-label={t('schemes.removeDay')}
                        onClick={() => update({ days: selected.days.filter((d) => d.date !== day.date) })}
                      >
                        <Trash2 />
                      </Button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </>
        )}
      </Panel>
    </div>
  )
}
