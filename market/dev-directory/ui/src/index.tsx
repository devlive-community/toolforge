import { useState } from 'react'
import { Badge, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Compass, ExternalLink, Plus, Search, Trash2 } from 'lucide-react'

interface Site {
  name: string
  url: string
  category: string
  description: string
}

interface ListOut {
  sites: Site[]
  total: number
}

interface CustomSite {
  name: string
  url: string
  category: string
}

const CATEGORIES = ['docs', 'tools', 'assets', 'learn', 'platform']

export default function DevDirectory() {
  const { t, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('')
  const [custom, saveCustom] = usePluginState<CustomSite[]>('custom', [])
  const [draft, setDraft] = useState<CustomSite>({ name: '', url: '', category: 'docs' })
  const [adding, setAdding] = useState(false)

  const needle = query.trim()
  const list = useDebouncedCall<ListOut>('list', { query: needle, category: category || undefined }, [needle, category])

  const open = (url: string) => {
    void host.openUrl(url)
  }

  const addCustom = () => {
    if (!draft.name.trim() || !draft.url.trim()) return
    const url = draft.url.trim().match(/^https?:\/\//) ? draft.url.trim() : `https://${draft.url.trim()}`
    saveCustom([...(custom ?? []), { ...draft, name: draft.name.trim(), url }])
    setDraft({ name: '', url: '', category: 'docs' })
    setAdding(false)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 items-center gap-2">
        <div className="w-44">
        <Select
          value={category}
          onValueChange={setCategory}
          options={[{ value: '', label: t('categories.all') }, ...CATEGORIES.map((key) => ({ value: key, label: t(`categories.${key}`) }))]}
          aria-label={t('api.list.category')}
        />
        </div>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          leading={<Search />}
          placeholder={t('api.list.query')}
          aria-label={t('api.list.query')}
          wrapperClassName="min-w-0 flex-1"
        />
      </div>
      <Panel
        icon={<Compass />}
        title={t('results.title')}
        className="min-h-0 flex-1"
        bodyClassName="min-h-0 overflow-auto py-1"
        extra={list.pending && <Spinner className="size-3.5" />}
      >
        {list.error ? (
          <p className="p-3 text-xs text-danger">{errorMessage(list.error)}</p>
        ) : (
          (list.result?.sites ?? []).map((site) => (
            <button
              key={site.url}
              type="button"
              onClick={() => open(site.url)}
              className="flex w-full items-center gap-3 border-t border-border px-3 py-2 text-left outline-none transition-colors first:border-t-0 hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
            >
              <span className="w-36 shrink-0 truncate text-[13px] font-medium text-fg">{site.name}</span>
              <span className="min-w-0 flex-1 truncate text-xs text-fg-muted">{t(`descriptions.${site.description}`)}</span>
              <Badge variant="neutral">{t(`categories.${site.category}`)}</Badge>
              <ExternalLink className="size-3.5 shrink-0 text-fg-subtle" />
            </button>
          ))
        )}
        <div className="border-t border-border px-3 py-2">
          <button
            type="button"
            onClick={() => setAdding((prev) => !prev)}
            className="flex items-center gap-1.5 text-xs text-primary-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <Plus className="size-3.5" />
            {t('custom.add')}
          </button>
          {adding && (
            <div className="mt-2 grid grid-cols-[1fr_1.4fr_120px_auto_auto] items-center gap-2">
              <Input size="sm" value={draft.name} onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))} placeholder={t('custom.name')} aria-label={t('custom.name')} />
              <Input size="sm" value={draft.url} onChange={(event) => setDraft((prev) => ({ ...prev, url: event.target.value }))} placeholder={t('custom.url')} aria-label={t('custom.url')} />
              <Select
                size="sm"
                value={draft.category}
                onValueChange={(value) => setDraft((prev) => ({ ...prev, category: value }))}
                options={CATEGORIES.map((key) => ({ value: key, label: t(`categories.${key}`) }))}
                aria-label={t('custom.category')}
              />
              <button type="button" onClick={addCustom} className="text-xs text-primary-fg outline-none focus-visible:ring-2 focus-visible:ring-ring">
                {t('custom.add')}
              </button>
              <button
                type="button"
                onClick={() => setAdding(false)}
                aria-label={t('common:close')}
                className="text-fg-subtle outline-none hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Trash2 className="size-3.5" />
              </button>
            </div>
          )}
          {(custom ?? []).map((site, index) => (
            <div key={`${site.url}-${index}`} className="mt-2 flex items-center gap-3 text-xs">
              <button type="button" onClick={() => open(site.url)} className="w-36 shrink-0 truncate text-left font-medium text-fg outline-none hover:text-primary-fg focus-visible:ring-2 focus-visible:ring-ring">
                {site.name}
              </button>
              <span className="min-w-0 flex-1 truncate font-mono text-[11px] text-fg-subtle">{site.url}</span>
              <Badge variant="neutral">{t(`categories.${site.category}`)}</Badge>
              <button
                type="button"
                aria-label={t('custom.delete') ?? ''}
                onClick={() => saveCustom((custom ?? []).filter((_, i) => i !== index))}
                className="text-fg-subtle outline-none hover:text-danger focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Trash2 className="size-3.5" />
              </button>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  )
}
