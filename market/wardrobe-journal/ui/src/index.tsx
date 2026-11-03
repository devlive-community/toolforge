import { useState } from 'react'
import { Badge, Button, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Plus, Shirt, Trash2 } from 'lucide-react'

interface Item {
  id: string
  name: string
  category: string
  color: string | null
  season: string | null
  state: string | null
  wearCount: number
}
interface Outfit {
  id: string
  name: string
  itemIds: string[]
}
interface SuggestOut {
  suggestions: { outfitId: string; name: string; status: string; dirty: string[] }[]
}

const CATEGORIES = ['top', 'bottom', 'shoes', 'outer', 'accessory']
const SEASONS = ['all', 'spring', 'summer', 'autumn', 'winter']

export default function WardrobeJournal() {
  const { t, errorMessage } = usePlugin()
  const [state, setState] = usePluginState<{ items: Item[]; outfits: Outfit[]; wearLog: Record<string, number> }>(
    'wardrobe',
    { items: [], outfits: [], wearLog: {} },
  )
  const [draft, setDraft] = useState({ name: '', category: 'top', season: 'all' })
  const [season, setSeason] = useState('')

  const addItem = () => {
    if (!draft.name.trim()) return
    const next = [
      ...(state?.items ?? []),
      { id: `i-${Date.now()}`, name: draft.name.trim(), category: draft.category, color: null, season: draft.season, state: 'clean', wearCount: 0 },
    ]
    setState({ items: next, outfits: state?.outfits ?? [], wearLog: state?.wearLog ?? {} })
    setDraft({ name: '', category: 'top', season: 'all' })
  }

  const removeItem = (id: string) => {
    setState({
      items: (state?.items ?? []).filter((item) => item.id !== id),
      outfits: (state?.outfits ?? []).map((outfit) => ({ ...outfit, itemIds: outfit.itemIds.filter((itemId) => itemId !== id) })),
      wearLog: state?.wearLog ?? {},
    })
  }

  const suggestArgs = (state?.items ?? []).length > 0 && (state?.outfits ?? []).length > 0
    ? { items: state!.items, outfits: state!.outfits, season: season || undefined }
    : null
  const suggest = useDebouncedCall<SuggestOut>(suggestArgs ? 'suggest' : null, suggestArgs, [season, state])
  const statsArgs = suggestArgs
  const stats = useDebouncedCall<{ items: { id: string; name: string; wearCount: number; state: string }[]; dirtyCount: number }>(
    suggestArgs ? 'stats' : null,
    statsArgs,
    [state],
  )

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel icon={<Shirt />} title={t('ui.items')} bodyClassName="space-y-2 p-3">
          <Input value={draft.name} onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))} placeholder={t('ui.name')} aria-label={t('ui.name')} />
          <div className="grid grid-cols-2 gap-2">
            <Select value={draft.category} onValueChange={(value) => setDraft((prev) => ({ ...prev, category: value }))} options={CATEGORIES.map((key) => ({ value: key, label: t(`categories.${key}`) }))} aria-label={t('ui.category')} />
            <Select value={draft.season} onValueChange={(value) => setDraft((prev) => ({ ...prev, season: value }))} options={SEASONS.map((key) => ({ value: key, label: t(`seasons.${key}`) }))} aria-label={t('seasons.all')} />
          </div>
          <Button size="sm" variant="secondary" onClick={addItem}>
            <Plus className="size-3.5" />
            {t('ui.add')}
          </Button>
          {(state?.items ?? []).length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('ui.empty')}</p>
          ) : (
            (state?.items ?? []).map((item) => (
              <div key={item.id} className="flex items-center gap-2 border-t border-border pt-1.5 text-xs">
                <span className="min-w-0 flex-1 truncate text-fg">{item.name}</span>
                <Badge variant={item.state === 'dirty' ? 'warning' : 'success'}>{t(`states.${item.state ?? 'clean'}`)}</Badge>
                <button type="button" onClick={() => removeItem(item.id)} aria-label={t('ui.delete')} className="text-fg-subtle outline-none hover:text-danger focus-visible:ring-2 focus-visible:ring-ring">
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))
          )}
        </Panel>
      </div>
      <div className="flex min-h-0 flex-col gap-3">
        <Panel title={t('ui.suggest')} bodyClassName="space-y-2 p-3" extra={suggest.pending && <Spinner className="size-3.5" />}>
          <Select value={season} onValueChange={setSeason} options={[{ value: '', label: t('seasons.all') }, ...SEASONS.map((key) => ({ value: key, label: t(`seasons.${key}`) }))]} aria-label={t('ui.suggest')} />
          {suggest.result ? (
            suggest.result.suggestions.map((suggestion) => (
              <div key={suggestion.outfitId} className="flex items-center justify-between rounded-control border border-border px-2.5 py-1.5 text-xs">
                <span className="text-fg">{suggestion.name}</span>
                <Badge variant={suggestion.status === 'ready' ? 'success' : 'warning'}>
                  {suggestion.status === 'ready' ? t('ui.ready') : `${t('ui.hasDirty')}: ${suggestion.dirty.join('、')}`}
                </Badge>
              </div>
            ))
          ) : (
            <p className="text-xs text-fg-subtle">{t('ui.empty')}</p>
          )}
          {suggest.error && <p className="text-xs text-danger">{errorMessage(suggest.error)}</p>}
        </Panel>
        <Panel title={t('ui.statsTitle')} bodyClassName="space-y-1 p-3" extra={stats.pending && <Spinner className="size-3.5" />}>
          {stats.result ? (
            <>
              <p className="text-xs text-fg-muted">{t('states.dirty')}: {stats.result.dirtyCount}</p>
              {stats.result.items.map((item) => (
                <p key={item.id} className="flex justify-between text-xs">
                  <span className="text-fg">{item.name}</span>
                  <span className="text-fg-muted tabular-nums">{t('ui.wear', { count: item.wearCount })}</span>
                </p>
              ))}
            </>
          ) : null}
        </Panel>
      </div>
    </div>
  )
}
