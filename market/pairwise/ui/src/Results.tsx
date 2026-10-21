import { useState } from 'react'
import { ArrowRight, ArrowUpRight, FileCode2, Grid2X2, ListChecks, X } from 'lucide-react'
import { Badge, Button, CodeEditor, Empty, Pagination, SegmentedControl, Spinner, Tooltip, cn } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import type { CaseRow, Page, PairRow, PairState, Summary } from './types'

const PAGE_SIZE = 25
const variants = { covered: 'success', uncovered: 'warning', infeasible: 'neutral', unknown: 'info' } as const

export function Results({ report, onUseModel }: { report: Summary; onUseModel: () => void }) {
  const { t, errorMessage } = usePlugin()
  const [tab, setTab] = useState<'cases' | 'pairs' | 'model'>('cases')
  const [state, setState] = useState<PairState | 'all'>('all')
  const [pair, setPair] = useState<PairRow | null>(null)
  const [casePage, setCasePage] = useState(1)
  const [pairPage, setPairPage] = useState(1)
  const cases = useDebouncedCall<Page<CaseRow>>(tab === 'cases' ? 'cases' : null,
    { handle: report.handle, offset: (casePage - 1) * PAGE_SIZE, limit: PAGE_SIZE, pairId: pair?.id }, [report.handle, tab, casePage, pair?.id])
  const pairs = useDebouncedCall<Page<PairRow>>(tab === 'pairs' ? 'pairs' : null,
    { handle: report.handle, offset: (pairPage - 1) * PAGE_SIZE, limit: PAGE_SIZE, state: state === 'all' ? undefined : state }, [report.handle, tab, pairPage, state])
  const pending = tab === 'cases' ? cases.pending : pairs.pending
  const currentCases = cases.pending ? null : cases.result
  const currentPairs = pairs.pending ? null : pairs.result
  const total = tab === 'cases' ? currentCases?.total ?? 0 : currentPairs?.total ?? 0
  const failure = pending ? null : tab === 'cases' ? cases.error : pairs.error

  const selectPair = (next: PairRow) => { setPair(next); setCasePage(1); setTab('cases') }
  const filterPairs = (next: PairState | 'all') => { setState(next); setPairPage(1); setTab('pairs') }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="grid shrink-0 grid-cols-3 gap-2">
        <Metric label={t('result.cases')} value={report.caseCount} />
        <Metric label={t('result.combinations')} value={report.combinations} />
        <Metric label={t('result.coverage')} value={report.coveragePercent === null ? t('result.notProven') : t('result.percent', { value: report.coveragePercent.toFixed(1) })} />
      </div>
      <div className="shrink-0 space-y-2">
        {report.status !== 'complete' && (
          <p role="status" className="rounded-control border border-warning/30 bg-warning/10 p-2.5 text-xs text-warning">
            {report.status === 'unsatisfiable' ? t('result.unsatisfiable') : t(`reason.${report.reason ?? 'uncovered'}`)}
          </p>
        )}
        <p className="text-xs text-fg-muted">{t(report.feasibilityComplete ? 'result.enumerated' : 'result.enumerationPartial', { scanned: report.enumerated, total: report.combinations, valid: report.validCombinations })}</p>
        <div className="grid grid-cols-4 gap-1.5">
          {(['covered', 'uncovered', 'infeasible', 'unknown'] as const).map((key) => (
            <button key={key} type="button" onClick={() => filterPairs(key)}
              className={cn('rounded-control border px-2 py-2 text-left outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring', state === key && tab === 'pairs' ? 'border-primary/40 bg-primary-soft' : 'border-border bg-surface-2')}>
              <span className="block text-[11px] text-fg-muted">{t(`pair.${key}`)}</span>
              <span className="mt-1 block font-mono text-base font-semibold text-fg">{report[key]}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="flex shrink-0 items-center justify-between gap-2">
        <SegmentedControl size="sm" value={tab} onValueChange={setTab} aria-label={t('result.views')} options={[
          { value: 'cases', label: <span className="flex items-center gap-1"><Grid2X2 className="size-3" />{t('result.matrix')}</span> },
          { value: 'pairs', label: <span className="flex items-center gap-1"><ListChecks className="size-3" />{t('result.pairs')}</span> },
          { value: 'model', label: <span className="flex items-center gap-1"><FileCode2 className="size-3" />{t('result.model')}</span> },
        ]} />
        {pending && tab !== 'model' && <Spinner className="size-3.5" />}
        {tab === 'pairs' && state !== 'all' && <Button size="sm" variant="ghost" onClick={() => filterPairs('all')}>{t('result.allPairs')}</Button>}
      </div>
      {tab === 'cases' && pair && (
        <div className="flex shrink-0 items-center gap-2 rounded-control bg-primary-soft px-2 py-1.5 text-xs text-fg">
          <span className="min-w-0 flex-1 break-all">{pair.left} = <code>{pair.leftValue}</code> · {pair.right} = <code>{pair.rightValue}</code></span>
          <Badge variant={variants[pair.state]}>{t(`pair.${pair.state}`)}</Badge>
          <Tooltip content={t('result.clearPair')}><Button size="icon-sm" variant="ghost" aria-label={t('result.clearPair')} onClick={() => { setPair(null); setCasePage(1) }}><X /></Button></Tooltip>
        </div>
      )}
      {tab === 'model' ? (
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          <Button size="sm" variant="soft" onClick={onUseModel}><ArrowUpRight />{t('result.useModel')}</Button>
          <div className="min-h-0 flex-1 rounded-control border border-border"><CodeEditor value={report.modelText} readOnly language="json" aria-label={t('result.model')} /></div>
        </div>
      ) : (
        <>
          {failure && <p role="alert" className="text-xs text-danger">{errorMessage(failure)}</p>}
          <div className={cn('min-h-0 flex-1 overflow-auto rounded-control border border-border', pending && 'opacity-60')}>
            {pending ? <div className="flex h-full items-center justify-center gap-2 text-xs text-fg-muted"><Spinner className="size-3.5" />{t('result.loading')}</div> : total === 0 ? <Empty icon={<ListChecks />} title={t(pair ? 'result.noWitnesses' : 'result.noRows')} /> : (
              <table className="w-full border-collapse text-left text-xs">
                {tab === 'cases' ? (
                  <>
                    <thead className="sticky top-0 bg-surface-2 text-fg-muted"><tr><th className="border-b border-border p-2 font-medium">{t('result.caseId')}</th>{report.factors.map((f) => <th key={f.name} className="border-b border-border p-2 font-medium">{f.name}</th>)}</tr></thead>
                    <tbody>{currentCases?.items.map((row) => <tr key={row.id} className="text-fg hover:bg-hover"><td className="border-b border-border/60 p-2 font-mono text-fg-subtle">{row.id}</td>{row.cells.map((cell, index) => <td key={index} className="max-w-64 break-all border-b border-border/60 p-2 font-mono" data-selectable>{cell}</td>)}</tr>)}</tbody>
                  </>
                ) : (
                  <>
                    <thead className="sticky top-0 bg-surface-2 text-fg-muted"><tr>{['pair', 'state', 'witnesses'].map((key) => <th key={key} className="border-b border-border p-2 font-medium">{t(`result.${key}`)}</th>)}</tr></thead>
                    <tbody>{currentPairs?.items.map((item) => <tr key={item.id} className="text-fg hover:bg-hover">
                      <td className="border-b border-border/60 p-2"><p className="break-all">{item.left} = <code className="font-mono">{item.leftValue}</code></p><p className="mt-1 break-all">{item.right} = <code className="font-mono">{item.rightValue}</code></p></td>
                      <td className="border-b border-border/60 p-2"><Badge variant={variants[item.state]}>{t(`pair.${item.state}`)}</Badge></td>
                      <td className="border-b border-border/60 p-2"><Button size="sm" variant="ghost" onClick={() => selectPair(item)} aria-label={t('result.findCases', { count: item.caseCount })}>{item.caseCount}<ArrowRight /></Button></td>
                    </tr>)}</tbody>
                  </>
                )}
              </table>
            )}
          </div>
          <Pagination page={tab === 'cases' ? casePage : pairPage} pageSize={PAGE_SIZE} total={total} onPageChange={tab === 'cases' ? setCasePage : setPairPage}
            prevLabel={t('page.previous')} nextLabel={t('page.next')} pageLabel={(page) => t('page.number', { page })} totalLabel={t('page.total', { count: total })} />
        </>
      )}
      <p className="shrink-0 text-[11px] text-fg-subtle">{t('result.boundary')}</p>
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string | number }) {
  return <div className="rounded-control border border-border bg-surface-2 p-2.5"><span className="block text-[11px] text-fg-muted">{label}</span><strong className="mt-1 block font-mono text-xl font-semibold tracking-tight text-fg">{value}</strong></div>
}
