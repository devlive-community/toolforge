import { useState } from 'react'
import { Input, Panel, Select, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Globe2, Search } from 'lucide-react'

interface Country {
  code: string
  nameZh: string
  nameEn: string
  capitalZh: string
  capitalEn: string
  continent: string
  currency: string
  callingCode: string
  populationM: number
}

interface ListOut {
  countries: Country[]
  total: number
}

const CONTINENTS = ['asia', 'europe', 'north-america', 'south-america', 'africa', 'oceania']

export default function WorldGeography() {
  const { t, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const [continent, setContinent] = useState('')

  const needle = query.trim()
  const args = { query: needle, continent: continent || undefined }
  const list = useDebouncedCall<ListOut>('list', needle || continent ? args : { query: '' }, [needle, continent])

  return (
    <Panel
      icon={<Globe2 />}
      title={t('api.list.summary')}
      className="min-h-0"
      bodyClassName="min-h-0 overflow-auto p-3"
      extra={list.pending && <Spinner className="size-3.5" />}
    >
      <div className="mb-3 flex items-center gap-2">
        <div className="w-44">
        <Select
          value={continent}
          onValueChange={setContinent}
          options={[{ value: '', label: t('continents.all') }, ...CONTINENTS.map((key) => ({ value: key, label: t(`continents.${key}`) }))]}
          aria-label={t('api.list.continent')}
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
      {list.error ? (
        <p className="text-xs text-danger">{errorMessage(list.error)}</p>
      ) : (
        <table className="w-full text-xs">
          <thead>
            <tr className="text-left text-fg-subtle">
              <th className="py-1 font-medium">{t('table.country')}</th>
              <th className="py-1 font-medium">{t('table.capital')}</th>
              <th className="py-1 font-medium">{t('table.continent')}</th>
              <th className="py-1 font-medium">{t('table.currency')}</th>
              <th className="py-1 text-right font-medium">{t('table.callingCode')}</th>
              <th className="py-1 text-right font-medium">{t('table.population')}</th>
            </tr>
          </thead>
          <tbody>
            {(list.result?.countries ?? []).map((country) => (
              <tr key={country.code} className="border-t border-border">
                <td className="py-1.5">
                  <span className="font-medium text-fg">{country.nameZh}</span>
                  <span className="ml-1.5 text-fg-subtle">{country.nameEn}</span>
                  <span className="ml-1.5 font-mono text-[10px] text-fg-subtle">{country.code}</span>
                </td>
                <td className="py-1.5 text-fg-muted">
                  {country.capitalZh}
                  <span className="ml-1 text-fg-subtle">{country.capitalEn}</span>
                </td>
                <td className="py-1.5 text-fg-muted">{t(`continents.${country.continent}`)}</td>
                <td className="py-1.5 font-mono text-fg-muted">{country.currency}</td>
                <td className="py-1.5 text-right font-mono tabular-nums">{country.callingCode}</td>
                <td className="py-1.5 text-right tabular-nums text-fg-muted">{t('population', { value: country.populationM })}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}
