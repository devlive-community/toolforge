import { useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Progress, Select, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Briefcase, Luggage, Minus, Plus, Trash2, X } from 'lucide-react'
import type { Bag, Exported, Summary, TemplateItem, Trip, TripItem } from './types'

const TEMPLATES: Record<string, TemplateItem[]> = {
  business: [
    { name: '笔记本电脑', category: '电子', unit: '台', base: 1, perPerson: false, perDay: false, weight: 1500 },
    { name: '充电器', category: '电子', unit: '个', base: 1, perPerson: true, perDay: false, weight: 120 },
    { name: '衬衫', category: '衣物', unit: '件', base: 2, perPerson: false, perDay: true, weight: 200 },
    { name: '洗漱包', category: '洗漱', unit: '套', base: 1, perPerson: false, perDay: false, weight: 300 },
    { name: '文件资料', category: '工作', unit: '份', base: 1, perPerson: true, perDay: false },
  ],
  shortTrip: [
    { name: '换洗内衣', category: '衣物', unit: '套', base: 2, perDay: true, weight: 100 },
    { name: '袜子', category: '衣物', unit: '双', base: 1, perDay: true, weight: 40 },
    { name: '充电宝', category: '电子', unit: '个', base: 1, perPerson: false, perDay: false, weight: 250 },
    { name: '零食', category: '食品', unit: '包', base: 1, perPerson: true, perDay: false },
    { name: '垃圾袋', category: '其他', unit: '个', base: 3, perPerson: false, perDay: false },
  ],
  family: [
    { name: '湿巾', category: '亲子', unit: '包', base: 1, perPerson: false, perDay: true, weight: 100 },
    { name: '儿童水杯', category: '亲子', unit: '个', base: 1, perPerson: true, perDay: false, weight: 150 },
    { name: '常用药', category: '药品', unit: '盒', base: 1, perPerson: false, perDay: false, weight: 80 },
    { name: '换洗衣物', category: '衣物', unit: '套', base: 2, perPerson: true, perDay: true, weight: 150 },
  ],
}

interface TripState {
  name: string
  people: number
  days: number
  bags: Bag[]
  items: TripItem[]
  templates: { id: string; name: string; items: TemplateItem[] }[]
}

const initialTrip = (): TripState => ({ name: '', people: 1, days: 1, bags: [], items: [], templates: [] })

function gramsText(value: number): string {
  return value >= 1000 ? `${Math.floor(value / 1000)}.${value % 1000} kg` : `${value} g`
}

export default function PackingList() {
  const { t, manifest, errorMessage } = usePlugin()
  const [trip, setTrip] = usePluginState<TripState>('trip', initialTrip())
  const [summary, setSummary] = useState<Summary | null>(null)
  const [failure, setFailure] = useState<string | null>(null)
  const [bagName, setBagName] = useState('')

  const recompute = async (next: TripState) => {
    setTrip(next)
    if (next.items.length === 0) {
      setSummary(null)
      return
    }
    try {
      const tripPayload: Trip = { people: next.people, days: next.days, bags: next.bags, items: next.items }
      setSummary(await callPlugin<Summary>(manifest.id, 'summary', tripPayload))
      setFailure(null)
    } catch (reason) {
      setFailure(errorMessage(reason))
    }
  }

  const setItems = (items: TripItem[]) => void recompute({ ...trip, items })

  const generate = async (key: string) => {
    try {
      const items = await callPlugin<TripItem[]>(manifest.id, 'generate', {
        template: TEMPLATES[key],
        people: trip.people,
        days: trip.days,
      })
      await recompute({ ...trip, items })
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const adjust = (id: string, delta: number) => {
    setItems(trip.items.map((item) => (item.id === id ? { ...item, packed: Math.max(0, Math.min(item.target, item.packed + delta)) } : item)))
  }

  const patch = (id: string, fields: Partial<TripItem>) => {
    setItems(trip.items.map((item) => (item.id === id ? { ...item, ...fields } : item)))
  }

  const addItem = async () => {
    const id = (await callPlugin<{ id: string }>(manifest.id, 'new_id', {})).id
    setItems([
      ...trip.items,
      { id, name: t('input.name'), category: '', unit: '', target: 1, packed: 0, owner: null, bagId: null, excluded: false, weight: null, note: null },
    ])
  }

  const addBag = async () => {
    if (!bagName.trim()) return
    const id = (await callPlugin<{ id: string }>(manifest.id, 'new_id', {})).id
    setBagName('')
    await recompute({ ...trip, bags: [...trip.bags, { id, name: bagName.trim(), selfWeight: null }] })
  }

  const saveTemplate = () => {
    const template = {
      id: Date.now().toString(),
      name: trip.name || t('input.trip'),
      items: trip.items.map((item) => ({
        name: item.name,
        category: item.category,
        unit: item.unit,
        base: item.target,
        perPerson: false,
        perDay: false,
        note: item.note,
        weight: item.weight,
      })),
    }
    setTrip({ ...trip, templates: [...trip.templates, template] })
    toast.success(t('common:saved'))
  }

  const exportReport = async (format: 'json' | 'text', save: boolean) => {
    if (!summary) return
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format, summary, tripName: trip.name })
      if (save) {
        const path = await host.dialog.saveFile(`packing.${out.ext}`, [{ name: out.ext.toUpperCase(), extensions: [out.ext] }])
        if (!path) return
        await host.fs.writeText(path, out.content)
        toast.success(t('common:saved'))
      } else {
        await host.clipboard.writeText(out.content)
        toast.success(t('common:saved'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const bagOptions = trip.bags.map((bag) => ({ value: bag.id, label: bag.name || bag.id }))

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(380px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        <Panel icon={<Briefcase />} title={t('input.trip')} bodyClassName="space-y-2 p-3">
          <Input
            value={trip.name}
            onChange={(event) => setTrip({ ...trip, name: event.target.value })}
            placeholder={t('input.tripNamePlaceholder')}
            size="sm"
            aria-label={t('input.tripName')}
          />
          <div className="flex items-center gap-2">
            <Input
              value={trip.people}
              onChange={(event) => void recompute({ ...trip, people: Number(event.target.value.replace(/[^\d]/g, '')) || 1 })}
              placeholder={t('input.people')}
              size="sm"
              aria-label={t('input.people')}
              wrapperClassName="w-20"
              inputMode="numeric"
            />
            <Input
              value={trip.days}
              onChange={(event) => void recompute({ ...trip, days: Number(event.target.value.replace(/[^\d]/g, '')) || 1 })}
              placeholder={t('input.days')}
              size="sm"
              aria-label={t('input.days')}
              wrapperClassName="w-20"
              inputMode="numeric"
            />
            <Select
              value=""
              onValueChange={(key) => void generate(key)}
              size="sm"
              aria-label={t('input.templates')}
              options={[
                { value: '', label: t('input.templates') },
                { value: 'business', label: t('input.template.business') },
                { value: 'shortTrip', label: t('input.template.shortTrip') },
                { value: 'family', label: t('input.template.family') },
              ]}
              className="w-32"
            />
            <Button size="sm" variant="ghost" onClick={saveTemplate} disabled={trip.items.length === 0}>
              {t('input.saveTemplate')}
            </Button>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Input value={bagName} onChange={(event) => setBagName(event.target.value)} placeholder={t('input.bagName')} size="sm" aria-label={t('input.bagName')} wrapperClassName="w-32" />
            <Button size="sm" variant="ghost" onClick={() => void addBag()} disabled={!bagName.trim()}>
              {t('input.addBag')}
            </Button>
            {trip.bags.map((bag) => (
              <Badge key={bag.id} variant="neutral">
                {bag.name || bag.id}
                <Tooltip content={t('input.remove')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('input.remove')} onClick={() => void recompute({ ...trip, bags: trip.bags.filter((item) => item.id !== bag.id), items: trip.items.map((item) => (item.bagId === bag.id ? { ...item, bagId: null } : item)) })}>
                    <X />
                  </Button>
                </Tooltip>
              </Badge>
            ))}
          </div>
        </Panel>

        <Panel
          icon={<Luggage />}
          title={t('input.items')}
          bodyClassName="space-y-2 p-3"
          actions={
            <Button size="sm" variant="ghost" onClick={() => void addItem()}>
              {t('input.addItem')}
            </Button>
          }
        >
          {trip.items.length === 0 && <p className="text-xs text-fg-subtle">{t('input.empty')}</p>}
          {trip.items.map((item) => (
            <div key={item.id} className={`space-y-1.5 rounded-control border border-border p-2 ${item.excluded ? 'opacity-50' : ''}`}>
              <div className="flex items-center gap-2">
                <Input value={item.name} onChange={(event) => patch(item.id, { name: event.target.value })} size="sm" aria-label={t('input.name')} wrapperClassName="min-w-0 flex-1" />
                <span className="flex shrink-0 items-center gap-1">
                  <Button size="icon-sm" variant="ghost" aria-label={t('input.packedMinus')} onClick={() => adjust(item.id, -1)}>
                    <Minus />
                  </Button>
                  <span className="w-12 text-center font-mono text-xs text-fg">{item.packed}/{item.target}</span>
                  <Button size="icon-sm" variant="ghost" aria-label={t('input.packedPlus')} onClick={() => adjust(item.id, 1)}>
                    <Plus />
                  </Button>
                </span>
                <label className="flex shrink-0 items-center gap-1 text-xs text-fg-muted">
                  <input type="checkbox" checked={item.excluded} onChange={(event) => patch(item.id, { excluded: event.target.checked, packed: event.target.checked ? 0 : item.packed })} className="accent-primary" />
                  {t('input.excluded')}
                </label>
                <Tooltip content={t('input.remove')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('input.remove')} onClick={() => setItems(trip.items.filter((entry) => entry.id !== item.id))}>
                    <Trash2 />
                  </Button>
                </Tooltip>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <Input value={item.category} onChange={(event) => patch(item.id, { category: event.target.value })} placeholder={t('input.category')} size="sm" aria-label={t('input.category')} wrapperClassName="w-24" />
                <Input value={item.unit} onChange={(event) => patch(item.id, { unit: event.target.value })} placeholder={t('input.unit')} size="sm" aria-label={t('input.unit')} wrapperClassName="w-16" />
                <Input value={item.weight ?? ''} onChange={(event) => patch(item.id, { weight: event.target.value === '' ? null : Number(event.target.value.replace(/[^\d]/g, '')) })} placeholder={t('input.weight')} size="sm" aria-label={t('input.weight')} wrapperClassName="w-28" inputMode="numeric" />
                <Select value={item.owner ?? ''} onValueChange={(owner) => patch(item.id, { owner: owner || null })} size="sm" aria-label={t('input.owner')} options={[{ value: '', label: t('input.owner') }]} className="w-24" />
                <Select value={item.bagId ?? ''} onValueChange={(bagId) => patch(item.id, { bagId: bagId || null })} size="sm" aria-label={t('input.bag')} options={[{ value: '', label: t('input.bag') }, ...bagOptions]} className="w-28" />
              </div>
            </div>
          ))}
        </Panel>
      </div>

      <Panel
        icon={<Luggage />}
        title={t('result.title')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        footer={
          summary && (
            <div className="flex w-full flex-wrap items-center gap-2">
              <Button size="sm" variant="soft" onClick={() => void exportReport('text', false)}>
                {t('export.text')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('json', false)}>
                {t('export.json')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="sm" onClick={() => void exportReport('text', true)}>
                  {t('export.saveText')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('json', true)}>
                  {t('export.saveJson')}
                </Button>
              </span>
            </div>
          )
        }
      >
        {failure && <p className="text-xs text-danger" role="alert">{failure}</p>}
        {!summary && !failure && (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<Luggage />} title={t('result.empty')} description={t('input.empty')} />
          </div>
        )}
        {summary && (
          <>
            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.progress')}</h3>
              <div className="flex items-center gap-2 text-xs text-fg-muted">
                <Progress value={summary.target > 0 ? (summary.packed / summary.target) * 100 : 0} className="flex-1" />
                <span className="font-mono text-fg">{summary.packed}/{summary.target}</span>
                <Badge variant="neutral">{t('result.excluded', { count: summary.excluded })}</Badge>
              </div>
            </section>

            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.byCategory')}</h3>
              {summary.categories.map((category) => (
                <div key={category.category} className="flex items-center gap-2 text-xs">
                  <span className="w-20 shrink-0 truncate text-fg">{category.category}</span>
                  <Progress value={category.target > 0 ? (category.packed / category.target) * 100 : 0} className="flex-1" />
                  <span className="font-mono text-fg-subtle">{category.packed}/{category.target}</span>
                </div>
              ))}
            </section>

            {summary.bags.length > 0 && (
              <section className="space-y-1.5">
                <h3 className="text-xs font-semibold text-fg">{t('result.byBag')}</h3>
                {summary.bags.map((bag) => (
                  <div key={bag.id} className="rounded-control border border-border bg-surface-2 p-2 text-xs">
                    <p className="font-medium text-fg">{bag.name || bag.id}</p>
                    <p className="text-fg-muted">
                      {t('result.packedWeight')}: {gramsText(bag.packedWeight)}
                      {bag.selfWeight !== null && bag.selfWeight !== undefined
                        ? ` · ${t('result.selfWeight')}: ${gramsText(bag.selfWeight)} · ${t('result.totalWeight')}: ${gramsText(bag.totalWeight ?? 0)}`
                        : ` · ${t('result.selfWeight')}: ${t('result.unknown')}`}
                    </p>
                  </div>
                ))}
              </section>
            )}

            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.unpacked')}</h3>
              {summary.unknownWeightItems > 0 && (
                <p className="text-xs text-warning">{t('result.incomplete', { count: summary.unknownWeightItems })}</p>
              )}
              {summary.unpacked.length === 0 ? (
                <p className="text-xs text-success" role="status">{t('result.allPacked')}</p>
              ) : (
                summary.unpacked.map((item) => (
                  <div key={item.id} className="rounded-control border border-border bg-surface-2 p-2 text-xs">
                    <span className="text-fg">{item.name}</span>
                    <Badge variant="warning">{t('input.excluded', { count: item.remaining })}</Badge>
                    {item.owner && <span className="ml-1 text-fg-subtle">{t('result.ownerTag', { owner: item.owner })}</span>}
                  </div>
                ))
              )}
            </section>
          </>
        )}
      </Panel>
    </div>
  )
}
