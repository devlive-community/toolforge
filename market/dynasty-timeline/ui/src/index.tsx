import { useState } from 'react'
import { Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Landmark, Search } from 'lucide-react'

interface Dynasty {
  key: string
  start: number
  end: number
  capital: string
  founder: string
  last: string
}

interface ListOut {
  dynasties: Dynasty[]
  total: number
}

interface LookupOut {
  year: number
  bce: boolean
  matches: Dynasty[]
}

function yearLabel(year: number): string {
  return year < 0 ? `${-year} BCE` : `AD ${year}`
}

export default function DynastyTimeline() {
  const { t, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const [yearText, setYearText] = useState('742') // tf-allow: 默认值

  const needle = query.trim()
  const list = useDebouncedCall<ListOut>('list', { query: needle }, [needle])
  const year = Number(yearText)
  const lookupArgs = yearText.trim() !== '' && Number.isInteger(year) && year !== 0 ? { year } : null
  const lookup = useDebouncedCall<LookupOut>(lookupArgs ? 'lookup' : null, lookupArgs, [yearText])

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1.4fr)_minmax(320px,1fr)] gap-3">
      <Panel
        icon={<Landmark />}
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
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-fg-subtle">
                <th className="py-1 font-medium">{t('dynasty')}</th>
                <th className="py-1 text-left font-medium">{t('table.range')}</th>
                <th className="py-1 font-medium">{t('table.capital')}</th>
                <th className="py-1 font-medium">{t('table.founder')}</th>
                <th className="py-1 font-medium">{t('table.last')}</th>
              </tr>
            </thead>
            <tbody>
              {(list.result?.dynasties ?? []).map((dynasty) => (
                <tr key={dynasty.key} className="border-t border-border">
                  <td className="py-1.5 font-medium text-fg">{t(`dynasties.${dynasty.key}`)}</td>
                  <td className="py-1.5 font-mono tabular-nums">
                    {yearLabel(dynasty.start)}{' – '}{yearLabel(dynasty.end)}
                  </td>
                  <td className="py-1.5 text-fg-muted">{t(`capitals.${dynasty.capital}`)}</td>
                  <td className="py-1.5 text-fg-muted">{t(`rulers.${dynasty.founder}`)}</td>
                  <td className="py-1.5 text-fg-muted">{t(`rulers.${dynasty.last}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
      <Panel
        icon={<Search />}
        title={t('lookup.title')}
        className="min-h-0 shrink-0"
        bodyClassName="space-y-2.5 p-3"
        extra={lookup.pending && <Spinner className="size-3.5" />}
      >
        <Input
          type="number"
          value={yearText}
          onChange={(event) => setYearText(event.target.value)}
          placeholder={t('api.lookup.year')}
          aria-label={t('api.lookup.year')}
        />
        {lookup.error ? (
          <p className="text-xs text-danger">{errorMessage(lookup.error)}</p>
        ) : lookup.result ? (
          lookup.result.matches.length > 0 ? (
            <div className="space-y-2">
              <p className="text-sm font-medium text-fg">
                {lookup.result.bce ? t('lookup.bce', { year: -lookup.result.year }) : t('lookup.ce', { year: lookup.result.year })}
              </p>
              {lookup.result.matches.map((dynasty) => (
                <div key={dynasty.key} className="rounded-control border border-border p-2.5">
                  <p className="text-sm font-medium text-fg">{t(`dynasties.${dynasty.key}`)}</p>
                  <p className="mt-0.5 font-mono text-[11px] text-fg-muted tabular-nums">
                    {yearLabel(dynasty.start)}{' – '}{yearLabel(dynasty.end)}
                  </p>
                  <p className="mt-1 text-xs text-fg-muted">
                    {t('table.capital')}: {t(`capitals.${dynasty.capital}`)}
                  </p>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-xs text-fg-subtle">{t('lookup.none')}</p>
          )
        ) : (
          <p className="text-xs text-fg-subtle">{t('api.lookup.year')}</p>
        )}
      </Panel>
    </div>
  )
}
