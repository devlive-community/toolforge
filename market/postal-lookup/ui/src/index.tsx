import { useState } from 'react'
import { Badge, Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { MapPin, Search } from 'lucide-react'

interface CityResult {
  cityZh: string
  cityEn: string
  postalCode: string
  areaCode: string
  adminCode: string
  matchedBy: string
}
interface LookupOut {
  results: CityResult[]
  total: number
}

export default function PostalLookup() {
  const { t, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const needle = query.trim()
  const lookup = useDebouncedCall<LookupOut>(needle ? 'lookup' : null, { query: needle }, [needle])

  return (
    <Panel icon={<MapPin />} title={t('api.lookup.summary')} className="min-h-0" bodyClassName="min-h-0 overflow-auto space-y-1 p-3" extra={lookup.pending && <Spinner className="size-3.5" />}>
      <Input value={query} onChange={(event) => setQuery(event.target.value)} leading={<Search />} placeholder={t('api.lookup.query')} aria-label={t('api.lookup.query')} />
      {lookup.error ? (
        <p className="text-xs text-danger">{errorMessage(lookup.error)}</p>
      ) : (
        (lookup.result?.results ?? []).map((result) => (
          <div key={result.adminCode + result.cityEn} className="flex items-center gap-3 border-t border-border py-2 first:border-t-0">
            <span className="w-20 shrink-0 text-sm font-medium text-fg">{result.cityZh}</span>
            <span className="min-w-0 flex-1 truncate text-xs text-fg-muted">{result.cityEn}</span>
            <span className="font-mono text-xs tabular-nums text-fg">{result.postalCode}</span>
            <span className="font-mono text-xs tabular-nums text-fg-muted">{result.areaCode}</span>
            <Badge variant="neutral">{t(`matched.${result.matchedBy}`)}</Badge>
          </div>
        ))
      )}
    </Panel>
  )
}
