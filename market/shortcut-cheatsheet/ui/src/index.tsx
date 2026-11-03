import { useState } from 'react'
import { Input, Panel, Select, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Keyboard, Plus, Search, Trash2 } from 'lucide-react'

interface Entry {
  app: string
  action: string
  category: string
  mac: string
  win: string
}

interface ListOut {
  entries: Entry[]
  total: number
  apps: string[]
}

interface CustomEntry {
  app: string
  action: string
  mac: string
  win: string
}

/** 键帽渲染：把 "Cmd+Shift+4" 拆成按键序列 */
function Keycaps({ keys }: { keys: string }) {
  return (
    <span className="inline-flex flex-wrap gap-1">
      {keys.split('+').map((key, index) => (
        <kbd
          key={`${key}-${index}`}
          className="rounded-control border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg shadow-card"
        >
          {key}
        </kbd>
      ))}
    </span>
  )
}

export default function ShortcutCheatSheet() {
  const { t, errorMessage } = usePlugin()
  const [app, setApp] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [custom, saveCustom] = usePluginState<CustomEntry[]>('custom', [])
  const [draft, setDraft] = useState<CustomEntry>({ app: '', action: '', mac: '', win: '' })
  const [adding, setAdding] = useState(false)

  const needle = query.trim()
  const args = { app: app ?? undefined, query: needle }
  const list = useDebouncedCall<ListOut>('list', needle || app ? args : { query: '' }, [app ?? '', needle])

  const addCustom = () => {
    if (!draft.app.trim() || !draft.action.trim()) return
    const next = [...(custom ?? []), { ...draft, app: draft.app.trim(), action: draft.action.trim() }]
    saveCustom(next)
    setDraft({ app: '', action: '', mac: '', win: '' })
    setAdding(false)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 items-center gap-2">
        <div className="w-48">
        <Select
          value={app ?? ''}
          onValueChange={(value) => setApp(value || null)}
          options={[{ value: '', label: t('apps.all') }, ...['vscode', 'chrome', 'jetbrains', 'office', 'macos', 'terminal'].map((key) => ({ value: key, label: t(`apps.${key}`) }))]}
          aria-label={t('api.list.app')}
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
        icon={<Keyboard />}
        title={t('results.title')}
        className="min-h-0 flex-1"
        bodyClassName="min-h-0 overflow-auto py-1"
        extra={list.pending && <Spinner className="size-3.5" />}
      >
        {list.error ? (
          <p className="p-3 text-xs text-danger">{errorMessage(list.error)}</p>
        ) : (
          (list.result?.entries ?? []).map((entry) => (
            <div key={`${entry.app}-${entry.action}`} className="flex items-center gap-3 border-t border-border px-3 py-2 first:border-t-0">
              <span className="w-40 shrink-0 truncate text-xs text-fg">{t(`actions.${entry.action}`)}</span>
              <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
                <span className="text-[11px] text-fg-subtle">{t('mac')}</span>
                <Keycaps keys={entry.mac} />
              </div>
              <div className="flex min-w-0 flex-1 items-center justify-between gap-2">
                <span className="text-[11px] text-fg-subtle">{t('win')}</span>
                <Keycaps keys={entry.win} />
              </div>
            </div>
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
            <div className="mt-2 grid grid-cols-[1fr_1fr_auto_auto_auto] items-center gap-2">
              <Input size="sm" value={draft.app} onChange={(event) => setDraft((prev) => ({ ...prev, app: event.target.value }))} placeholder={t('custom.app')} aria-label={t('custom.app')} />
              <Input size="sm" value={draft.action} onChange={(event) => setDraft((prev) => ({ ...prev, action: event.target.value }))} placeholder={t('custom.action')} aria-label={t('custom.action')} />
              <Input size="sm" value={draft.mac} onChange={(event) => setDraft((prev) => ({ ...prev, mac: event.target.value }))} placeholder={t('mac')} aria-label={t('mac')} />
              <Input size="sm" value={draft.win} onChange={(event) => setDraft((prev) => ({ ...prev, win: event.target.value }))} placeholder={t('win')} aria-label={t('win')} />
              <button type="button" onClick={addCustom} className="text-xs text-primary-fg outline-none focus-visible:ring-2 focus-visible:ring-ring">
                {t('custom.add')}
              </button>
            </div>
          )}
          {(custom ?? []).map((entry, index) => (
            <div key={`${entry.app}-${entry.action}-${index}`} className="mt-2 flex items-center gap-3 text-xs">
              <span className="w-40 shrink-0 truncate text-fg">{entry.action}</span>
              <span className="text-fg-subtle">{entry.app}</span>
              <Keycaps keys={entry.mac} />
              <Keycaps keys={entry.win} />
              <button
                type="button"
                aria-label={t('custom.delete') ?? ''}
                onClick={() => saveCustom((custom ?? []).filter((_, i) => i !== index))}
                className="ml-auto text-fg-subtle outline-none hover:text-danger focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Trash2 className="size-3.5" />
              </button>
            </div>
          ))}
          {(!custom || custom.length === 0) && !adding && (
            <p className="mt-1 text-[11px] text-fg-subtle">{t('custom.empty')}</p>
          )}
        </div>
      </Panel>
    </div>
  )
}
