import { useState } from 'react'
import { Badge, Input, Panel, Spinner } from '@toolforge/ui'
import { host, ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileSearch, Search, ShieldAlert } from 'lucide-react'

interface Entry {
  extension: string
  category: string
  description: string
  mime: string
  risk: string
}

interface ListOut {
  entries: Entry[]
  total: number
}

interface IdentifyOut {
  nominal: string
  real: string | null
  magicHex: string
  matchesNominal: boolean | null
  risk: string | null
}

export default function FileExtensionEncyclopedia() {
  const { t, errorMessage } = usePlugin()
  const { call } = usePlugin()
  const [query, setQuery] = useState('')
  const [path, setPath] = useState('')
  const [identifying, setIdentifying] = useState(false)
  const [identifyError, setIdentifyError] = useState<string | null>(null)
  const [identified, setIdentified] = useState<IdentifyOut | null>(null)

  const needle = query.trim()
  const list = useDebouncedCall<ListOut>('list', { query: needle }, [needle])

  const pickAndIdentify = async () => {
    // 路径来自系统对话框选择，进入 fs:user-selected 授权范围
    const picked = await host.dialog.openFile()
    if (!picked) return
    setPath(picked)
    setIdentifying(true)
    setIdentifyError(null)
    setIdentified(null)
    try {
      setIdentified(await call<IdentifyOut>('identify', { path: picked }))
    } catch (error) {
      setIdentifyError(String(error))
    } finally {
      setIdentifying(false)
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel
        icon={<Search />}
        title={t('api.list.summary')}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-1 p-3"
        extra={list.pending && <Spinner className="size-3.5" />}
      >
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          leading={<Search />}
          placeholder={t('api.list.query')}
          aria-label={t('api.list.query')}
        />
        {list.error ? (
          <p className="text-xs text-danger">{errorMessage(list.error)}</p>
        ) : (
          (list.result?.entries ?? []).map((entry) => (
            <div key={entry.extension} className="flex items-center gap-2 border-t border-border py-1.5 first:border-t-0">
              <span className="w-16 shrink-0 font-mono text-[13px] font-semibold text-fg">{entry.extension}</span>
              <div className="min-w-0 flex-1">
                <p className="truncate text-xs text-fg-muted">{t(`descriptions.desc_${entry.extension.replace('.', '_')}`)}</p>
                <p className="truncate font-mono text-[11px] text-fg-subtle">{entry.mime}</p>
              </div>
              <Badge variant={entry.risk === 'safe' ? 'success' : entry.risk === 'executable' ? 'danger' : 'neutral'}>
                {t(`risks.${entry.risk}`)}
              </Badge>
            </div>
          ))
        )}
      </Panel>
      <Panel
        icon={<ShieldAlert />}
        title={t('identify.title')}
        className="min-h-0 shrink-0"
        bodyClassName="space-y-2.5 p-3"
        extra={identifying && <Spinner className="size-3.5" />}
      >
        <button
          type="button"
          onClick={() => void pickAndIdentify()}
          className="flex h-24 w-full flex-col items-center justify-center gap-1.5 rounded-control border border-dashed border-border text-xs text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
        >
          <FileSearch className="size-5" />
          {t('identify.pick')}
        </button>
        {identifyError && <p className="text-xs text-danger">{identifyError}</p>}
        {path && !identifyError && <p className="truncate font-mono text-[11px] text-fg-subtle">{path}</p>}
        {identified && (
          <div className="space-y-1">
            <ValueRow label={t('identify.nominal')} value={identified.nominal || '—'} labelWidth="w-24" />
            <ValueRow
              label={t('identify.real')}
              value={identified.real ? t(`magic.${identified.real}`) : t('identify.unknown')}
              labelWidth="w-24"
            />
            {identified.risk && (
              <div className="pl-[calc(6rem+0.5rem)]">
                <Badge variant={identified.risk === 'executable' ? 'danger' : identified.risk === 'safe' ? 'success' : 'neutral'}>
                  {t(`risks.${identified.risk}`)}
                </Badge>
              </div>
            )}
            <ValueRow label={t('identify.magic')} value={identified.magicHex} labelWidth="w-24" />
            {identified.matchesNominal === false && (
              <p className="text-xs font-medium text-danger">{t('identify.matchBad')}</p>
            )}
            {identified.matchesNominal === true && <p className="text-xs text-success">{t('identify.matchOk')}</p>}
          </div>
        )}
      </Panel>
    </div>
  )
}
