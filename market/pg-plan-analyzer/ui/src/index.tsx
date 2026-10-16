import { useEffect, useMemo, useRef, useState } from 'react'
import {
  Badge,
  Button,
  CodeEditor,
  Empty,
  Modal,
  Panel,
  Progress,
  SegmentedControl,
  cn,
  toast,
} from '@toolforge/ui'
import { callPlugin, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Flame, Gauge, Network, TriangleAlert, Upload } from 'lucide-react'
import type { Diag, Exported, Hotspot, PlanNode, Report } from './types'

const SAMPLE = `[
  {
    "Plan": {
      "Node Type": "Seq Scan",
      "Relation Name": "orders",
      "Startup Cost": 0.0,
      "Total Cost": 1200.5,
      "Plan Rows": 5000,
      "Actual Total Time": 15.2,
      "Actual Rows": 120000,
      "Actual Loops": 1
    },
    "Execution Time": 16.1
  }
]`

function flatten(node: PlanNode, out: PlanNode[] = []): PlanNode[] {
  out.push(node)
  for (const child of node.children) flatten(child, out)
  return out
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="w-40 shrink-0 text-fg-subtle">{label}</span>
      <span className="min-w-0 flex-1 font-mono break-all text-fg" data-selectable>
        {value}
      </span>
    </div>
  )
}

function number(value: number | null, digits = 3): string | null {
  return value === null ? null : value.toFixed(digits)
}

function NodeDetail({ node }: { node: PlanNode }) {
  const { t, errorMessage } = usePlugin()
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="info">{node.nodeType}</Badge>
        {node.parallel && <Badge variant="neutral">{t('detail.parallel')}</Badge>}
        {node.exclusiveMs !== null && (
          <Badge variant="success">{`${t('detail.exclusive')} ${node.exclusiveMs.toFixed(3)}`}</Badge>
        )}
      </div>
      {node.notes.map((note, index) => (
        <p key={index} className="flex items-start gap-1.5 text-xs text-warning">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          {errorMessage(note)}
        </p>
      ))}
      <div className="space-y-1">
        <Row label={t('detail.relation')} value={node.relation ?? '—'} />
        {node.indexName && <Row label={t('detail.indexName')} value={node.indexName} />}
        {node.joinType && <Row label={t('detail.joinType')} value={node.joinType} />}
        {node.filter && <Row label={t('detail.filter')} value={node.filter} />}
        {node.indexCond && <Row label={t('detail.indexCond')} value={node.indexCond} />}
        {node.hashCond && <Row label={t('detail.hashCond')} value={node.hashCond} />}
        {node.joinFilter && <Row label={t('detail.joinFilter')} value={node.joinFilter} />}
        {node.sortKey.length > 0 && <Row label={t('detail.sortKey')} value={node.sortKey.join(', ')} />}
        <Row label={t('detail.startupCost')} value={number(node.startupCost, 2) ?? '—'} />
        <Row label={t('detail.totalCost')} value={number(node.totalCost, 2) ?? '—'} />
        <Row label={t('detail.planRows')} value={number(node.planRows, 0) ?? '—'} />
        <Row label={t('detail.planWidth')} value={node.planWidth === null ? '—' : String(node.planWidth)} />
        <Row label={t('detail.actualTotal')} value={number(node.actualTotal) ?? '—'} />
        <Row label={t('detail.actualRows')} value={number(node.actualRows, 0) ?? '—'} />
        <Row label={t('detail.loops')} value={number(node.loops, 0) ?? '—'} />
        <Row label={t('detail.inclusive')} value={number(node.inclusiveMs) ?? '—'} />
        <Row label={t('detail.exclusive')} value={number(node.exclusiveMs) ?? '—'} />
        {node.rowsRemovedByFilter !== null && (
          <Row label={t('detail.rowsRemoved')} value={number(node.rowsRemovedByFilter, 0) ?? '—'} />
        )}
        {node.tempWritten !== null && <Row label={t('detail.tempWritten')} value={number(node.tempWritten, 0) ?? '—'} />}
        {node.sharedHit !== null && <Row label={t('detail.sharedHit')} value={number(node.sharedHit, 0) ?? '—'} />}
        {node.sharedRead !== null && <Row label={t('detail.sharedRead')} value={number(node.sharedRead, 0) ?? '—'} />}
        {node.heapFetches !== null && <Row label={t('detail.heapFetches')} value={number(node.heapFetches, 0) ?? '—'} />}
      </div>
    </div>
  )
}

function HotspotList({ hotspots }: { hotspots: Hotspot[] }) {
  const { t } = usePlugin()
  if (hotspots.length === 0) return <p className="text-xs text-fg-subtle">{t('hotspots.empty')}</p>
  return (
    <div className="space-y-1">
      {hotspots.map((hotspot, index) => (
        <div key={`${hotspot.statement}-${hotspot.nodeId}-${index}`} className="flex items-center gap-2 text-xs">
          <Flame className="size-3.5 shrink-0 text-warning" />
          <span className="min-w-0 flex-1 truncate font-mono text-fg" data-selectable>
            {hotspot.label}
          </span>
          <span className="shrink-0 text-fg-subtle">{t(`hotspots.${hotspot.kind}`)}</span>
          <span className="shrink-0 font-mono text-fg">{hotspot.value.toFixed(3)}</span>
        </div>
      ))}
    </div>
  )
}

export default function PgPlanAnalyzer() {
  const { t, manifest, errorMessage } = usePlugin()
  const loading = useTask<Report>()
  const [pasted, setPasted] = useState<Report | null>(null)
  const [statementIndex, setStatementIndex] = useState(0)
  const [selected, setSelected] = useState(0)
  const [failure, setFailure] = useState<Diag | null>(null)
  const [pasteOpen, setPasteOpen] = useState(false)
  const [pasteText, setPasteText] = useState('')
  const [dropping, setDropping] = useState(false)

  useEffect(() => {
    if (loading.status === 'failed' && loading.error) {
      toast.error(errorMessage(loading.error))
    }
  }, [loading.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const taskReport = loading.status === 'succeeded' ? loading.result : null
  const report = pasted ?? taskReport

  const load = (path: string) => {
    setFailure(null)
    setPasted(null)
    void loading.start('load', { path })
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && loadRef.current(paths[0]), over: setDropping })
    return () => {
      void off.then((unlisten) => unlisten())
    }
  }, [])

  const openFile = async () => {
    const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ['json', 'txt'] }])
    if (path) load(path)
  }

  const parsePaste = async () => {
    try {
      const out = await callPlugin<Report>(manifest.id, 'parse', { text: pasteText })
      setPasted(out)
      setStatementIndex(0)
      setSelected(0)
      setFailure(null)
      setPasteOpen(false)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const exportReport = async (format: 'markdown' | 'json') => {
    if (!report) return null
    return callPlugin<Exported>(manifest.id, 'export', { handle: report.handle, format })
  }

  const copyReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await exportReport(format)
      if (!out) return
      await host.clipboard.writeText(out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const saveReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await exportReport(format)
      if (!out) return
      const path = await host.dialog.saveFile(`pg-plan.${out.ext}`, [
        { name: out.ext.toUpperCase(), extensions: [out.ext] },
      ])
      if (!path) return
      await host.fs.writeText(path, out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const statement = report ? report.statements[Math.min(statementIndex, report.statements.length - 1)] : null
  const nodes = useMemo(() => (statement?.root ? flatten(statement.root) : []), [statement])
  const selectedNode = nodes.length > 0 ? nodes[Math.min(selected, nodes.length - 1)] : null
  const statementHotspots = useMemo(
    () => (report ? report.hotspots.filter((hotspot) => hotspot.statement === statement?.index) : []),
    [report, statement],
  )

  if (!report) {
    return (
      <>
        <div className="flex h-full flex-col items-center justify-center gap-3">
          <Empty icon={<Gauge />} title={t('open.emptyTitle')} description={t('open.emptyHint')} />
          <div className="flex items-center gap-2">
            <Button onClick={() => void openFile()}>
              <Upload />
              {t('open.openFile')}
            </Button>
            <Button variant="soft" onClick={() => setPasteOpen(true)}>
              {t('open.paste')}
            </Button>
          </div>
          {loading.running && (
            <div className="flex w-72 items-center gap-3">
              <Progress
                value={loading.progress ? (loading.progress.done / Math.max(loading.progress.total, 1)) * 100 : null}
                aria-label={t('open.loading')}
              />
              <span className="shrink-0 text-xs text-fg-muted">{t('open.loading')}</span>
              <Button size="sm" onClick={() => void loading.cancel()}>
                {t('open.cancel')}
              </Button>
            </div>
          )}
          {dropping && <p className="text-xs text-fg-muted">{t('open.drop')}</p>}
          {failure && (
            <p className="flex items-center gap-1.5 text-xs text-danger">
              <TriangleAlert className="size-3.5" />
              {errorMessage(failure)}
            </p>
          )}
        </div>
        <Modal open={pasteOpen} onOpenChange={setPasteOpen} size="lg" title={t('open.pasteTitle')}>
          <div className="space-y-3">
            <div className="h-64 overflow-hidden rounded-control border border-border">
              <CodeEditor
                value={pasteText}
                onChange={setPasteText}
                language="json"
                placeholder={SAMPLE}
                aria-label={t('open.pasteTitle')}
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button onClick={() => void parsePaste()} disabled={pasteText.trim().length === 0}>
                {t('open.parse')}
              </Button>
            </div>
          </div>
        </Modal>
      </>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="shrink-0 space-y-2 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
          {statement?.hasActual ? (
            <Badge variant="success">{t('overview.hasActual')}</Badge>
          ) : (
            <Badge variant="neutral">{t('overview.planOnly')}</Badge>
          )}
          {statement?.executionTime !== null && statement?.executionTime !== undefined && (
            <span className="text-fg">
              {t('overview.execution')}: <span className="font-mono">{t('detail.msValue', { value: statement.executionTime.toFixed(3) })}</span>
            </span>
          )}
          {statement?.planningTime !== null && statement?.planningTime !== undefined && (
            <span className="text-fg">
              {t('overview.planning')}: <span className="font-mono">{t('detail.msValue', { value: statement.planningTime.toFixed(3) })}</span>
            </span>
          )}
          <span className="text-fg-subtle">
            {t('overview.nodes')}: <span className="font-mono text-fg">{statement?.nodeCount ?? 0}</span>
          </span>
          <span className="ml-auto flex items-center gap-2">
            <Button size="sm" variant="soft" onClick={() => void copyReport('markdown')}>
              {t('export.markdown')}
            </Button>
            <Button size="sm" variant="soft" onClick={() => void copyReport('json')}>
              {t('export.json')}
            </Button>
            <Button size="sm" onClick={() => void saveReport('markdown')}>
              {t('export.saveMd')}
            </Button>
            <Button size="sm" onClick={() => void saveReport('json')}>
              {t('export.saveJson')}
            </Button>
          </span>
        </div>
        {statement?.statement && (
          <p className="truncate font-mono text-xs text-fg-muted" data-selectable>
            {statement.statement}
          </p>
        )}
        {report.statements.length > 1 && (
          <SegmentedControl
            value={String(statement?.index ?? 0)}
            onValueChange={(value) => {
              setStatementIndex(Number(value))
              setSelected(0)
            }}
            options={report.statements.map((item) => ({
              value: String(item.index),
              label: `${t('overview.statement')} ${item.index + 1}`,
            }))}
            size="sm"
          />
        )}
      </section>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(320px,1fr)_minmax(0,1fr)] gap-3">
        <Panel icon={<Network />} title={t('tree.title')}>
          <div className="h-full min-h-0 overflow-y-auto">
            {nodes.length === 0 && (
              <div className="flex h-full items-center justify-center">
                <Empty icon={<Network />} title={t('tree.empty')} />
              </div>
            )}
            {nodes.map((node) => (
              <button
                key={node.id}
                type="button"
                onClick={() => setSelected(node.id)}
                className={cn(
                  'flex w-full items-center gap-2 rounded-control px-2 py-1.5 text-left',
                  node.id === selectedNode?.id ? 'bg-hover' : 'hover:bg-hover',
                )}
                style={{ paddingLeft: `${8 + node.depth * 16}px` }}
              >
                <span
                  className={cn(
                    'min-w-0 flex-1 truncate font-mono text-xs',
                    node.nodeType.includes('Scan') ? 'text-fg' : 'text-fg-muted',
                  )}
                  title={node.label}
                >
                  {node.label}
                </span>
                {node.notes.length > 0 && <TriangleAlert className="size-3.5 shrink-0 text-warning" />}
                {node.exclusiveMs !== null ? (
                  <span className="shrink-0 font-mono text-xs text-fg-subtle">{t('detail.msValue', { value: node.exclusiveMs.toFixed(2) })}</span>
                ) : (
                  node.totalCost !== null && (
                    <span className="shrink-0 font-mono text-xs text-fg-subtle">
                      {t('tree.cost', { cost: node.totalCost.toFixed(2) })}
                    </span>
                  )
                )}
              </button>
            ))}
          </div>
        </Panel>

        <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
          <Panel
            icon={<Gauge />}
            title={t('detail.title')}
            extra={
              selectedNode && (
                <span className="font-mono text-xs text-fg-subtle">
                  {selectedNode.actualRows !== null && selectedNode.loops !== null
                    ? t('tree.perLoop', { rows: selectedNode.actualRows, loops: selectedNode.loops })
                    : t('tree.noMetrics')}
                </span>
              )
            }
          >
            <div className="h-full min-h-0 overflow-y-auto">
              {selectedNode ? (
                <NodeDetail node={selectedNode} />
              ) : (
                <p className="text-xs text-fg-subtle">{t('detail.empty')}</p>
              )}
            </div>
          </Panel>
          <section className="rounded-card border border-border bg-surface p-3 shadow-card">
            <p className="mb-2 flex items-center gap-1.5 text-xs font-medium text-fg-muted">
              <Flame className="size-3.5" />
              {t('hotspots.title')}
            </p>
            <HotspotList hotspots={statementHotspots} />
          </section>
        </div>
      </div>
    </div>
  )
}
