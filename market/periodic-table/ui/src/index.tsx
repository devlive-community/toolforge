import { useState } from 'react'
import { Badge, Input, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FlaskConical, Search, Sigma } from 'lucide-react'

interface Element {
  number: number
  symbol: string
  nameZh: string
  nameEn: string
  weight: number
  category: string
  shells: number
}
interface ListOut {
  elements: Element[]
  total: number
}
interface MassPart {
  symbol: string
  count: number
  subtotal: number
}
interface MolarOut {
  formula: string
  molarMass: number
  parts: MassPart[]
}

export default function PeriodicTable() {
  const { t, errorMessage } = usePlugin()
  const [tab, setTab] = useState<'table' | 'molar'>('table')
  const [query, setQuery] = useState('')
  const needle = query.trim()
  const list = useDebouncedCall<ListOut>('list', { query: needle }, [needle])
  const [formula, setFormula] = useState('H2O') // tf-allow: 默认值
  const molar = useDebouncedCall<MolarOut>(tab === 'molar' && formula.trim() ? 'molar_mass' : null, { formula }, [formula])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('tab.label')}
        options={[
          { value: 'table', label: t('tab.table') },
          { value: 'molar', label: t('tab.molar') },
        ]}
      />
      {tab === 'table' ? (
        <Panel icon={<FlaskConical />} title={t('api.list.summary')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-1 p-3" extra={list.pending && <Spinner className="size-3.5" />}>
          <Input value={query} onChange={(event) => setQuery(event.target.value)} leading={<Search />} placeholder={t('api.list.query')} aria-label={t('api.list.query')} />
          {list.error ? (
            <p className="text-xs text-danger">{errorMessage(list.error)}</p>
          ) : (
            (list.result?.elements ?? []).map((element) => (
              <div key={element.number} className="flex items-center gap-2 border-t border-border py-1.5 first:border-t-0">
                <span className="w-8 shrink-0 text-right font-mono text-[11px] text-fg-subtle tabular-nums">{element.number}</span>
                <Badge variant={element.category === 'metal' ? 'primary' : element.category === 'noble' ? 'neutral' : 'success'}>{element.symbol}</Badge>
                <span className="w-16 shrink-0 text-xs font-medium text-fg">{element.nameZh}</span>
                <span className="min-w-0 flex-1 truncate text-xs text-fg-muted">{element.nameEn}</span>
                <span className="font-mono text-xs tabular-nums text-fg-muted">{element.weight}</span>
              </div>
            ))
          )}
        </Panel>
      ) : (
        <Panel icon={<Sigma />} title={t('molar.title')} className="min-h-0 flex-1" bodyClassName="space-y-2.5 p-3" extra={molar.pending && <Spinner className="size-3.5" />}>
          <Input value={formula} onChange={(event) => setFormula(event.target.value)} placeholder="H2O" aria-label={t('api.molar_mass.formula')} />
          {molar.error ? (
            <p className="text-xs text-danger">{errorMessage(molar.error)}</p>
          ) : (
            molar.result && (
              <>
                <p className="text-2xl font-semibold text-fg">
                  {molar.result.molarMass} <span className="text-sm font-normal text-fg-muted">{"g/mol"}</span>
                </p>
                {molar.result.parts.map((part) => (
                  <div key={part.symbol} className="flex justify-between text-xs">
                    <span className="font-mono text-fg">{part.symbol} {t('molar.count', { count: part.count })}</span>
                    <span className="text-fg-muted tabular-nums">{part.subtotal}</span>
                  </div>
                ))}
              </>
            )
          )}
        </Panel>
      )}
    </div>
  )
}
