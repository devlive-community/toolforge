import { useEffect, useEffectEvent, useRef, useState } from 'react'
import { Badge, Button, DropdownMenu, Empty, Panel, SegmentedControl, Select, Spinner, Switch, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDownToLine, ArrowLeftRight, ChartLine, Plus, X } from 'lucide-react'
import { ChartView } from './ChartView'
import { DataPanel, type Source } from './DataPanel'
import { pngDataUrl, svgMarkup } from './exportChart'
import { SAMPLE } from './sample'
import { PALETTE, type Chart, type Config, type Format, type HeaderMode, type Note, type Plot, type Side, type Summary, type XType } from './types'

/** 最多同时绘制的系列数（与后端一致） */
const MAX_SERIES = 12

const fromSuggestion = (summary: Summary): Config => ({
  chart: summary.suggestion.chart,
  x: summary.suggestion.x,
  xType: 'auto',
  series: summary.suggestion.series.map((column) => ({ column, axis: 'left' as Side })),
  logLeft: false,
  logRight: false,
})

const FILE_FILTERS = [{ name: 'CSV / TSV / JSON', extensions: ['csv', 'tsv', 'tab', 'txt', 'json', 'jsonl', 'ndjson', 'log'] }]

function QuickPlot() {
  const { t, call, errorMessage } = usePlugin()
  const [text, setText] = useState('')
  const [format, setFormat] = useState<Format>('auto')
  const [header, setHeader] = useState<HeaderMode>('auto')
  const [source, setSource] = useState<Source>({ kind: 'text' })
  const load = useTask<Summary>()
  const svg = useRef<SVGSVGElement>(null)

  const pasted = useDebouncedCall<Summary>(source.kind === 'text' && text.trim() ? 'parse' : null, source.kind === 'text' && text.trim() ? { text, format, header } : null, [source.kind, text, format, header])
  const summary = source.kind === 'file' ? (load.status === 'succeeded' ? load.result : null) : text.trim() ? pasted.result : null
  const dataError = source.kind === 'file' ? (load.status === 'failed' && load.error ? errorMessage(load.error) : null) : text.trim() && pasted.error ? errorMessage(pasted.error) : null

  // 列结构变化（新数据）时套用推荐配置；只是改了数值则保留用户的选择
  const [config, setConfig] = useState<Config | null>(null)
  const [applied, setApplied] = useState('')
  const signature = summary ? summary.columns.map((c) => `${c.name}:${c.kind}`).join('|') : ''
  if (summary && signature !== applied) {
    setApplied(signature)
    setConfig(fromSuggestion(summary))
  }

  const ready = summary && config && config.series.length > 0
  const plotted = useDebouncedCall<Plot>(ready ? 'plot' : null, ready ? { token: summary.token, ...config } : null, [summary?.token, config])
  const plot = ready ? plotted.result : null

  const openPath = (path: string, nextFormat = format, nextHeader = header) => {
    setSource({ kind: 'file', path })
    void load.start('load', { path, format: nextFormat, header: nextHeader })
  }

  const openFile = async () => {
    const path = await host.dialog.openFile(FILE_FILTERS)
    if (path) openPath(path)
  }

  // 拖入文件直接打开
  const onDrop = useEffectEvent((path: string) => openPath(path))
  useEffect(() => {
    let unlisten: (() => void) | undefined
    let alive = true
    void host.onFileDrop({ drop: (paths) => paths[0] && onDrop(paths[0]) }).then((fn) => {
      if (alive) unlisten = fn
      else fn()
    })
    return () => {
      alive = false
      unlisten?.()
    }
  }, [])

  const changeFormat = (next: Format) => {
    setFormat(next)
    if (source.kind === 'file') openPath(source.path, next, header)
  }
  const changeHeader = (next: HeaderMode) => {
    setHeader(next)
    if (source.kind === 'file') openPath(source.path, format, next)
  }

  const update = (patch: Partial<Config>) => setConfig((prev) => (prev ? { ...prev, ...patch } : prev))
  const numeric = summary?.columns.filter((c) => c.kind === 'number') ?? []
  const unused = numeric.filter((c) => !config?.series.some((s) => s.column === c.index))
  const hasRight = config?.series.some((s) => s.axis === 'right') ?? false

  const exportChart = async (kind: 'png' | 'svg') => {
    if (!svg.current) return
    try {
      const path = await host.dialog.saveFile(`chart.${kind}`, [{ name: kind.toUpperCase(), extensions: [kind] }])
      if (!path) return
      if (kind === 'svg') await host.fs.writeText(path, svgMarkup(svg.current))
      else await call('save_png', { path, data: await pngDataUrl(svg.current) })
      toast.success(t('chart.saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const notes: Note[] = [...(summary?.notes ?? []), ...(plot?.notes ?? [])]
  const seriesNotes = plot?.series.flatMap((s) => [
    ...(s.skipped > 0 ? [t('notes.skipped', { name: s.name, count: s.skipped })] : []),
    ...(s.sampled ? [t('notes.sampled', { name: s.name, shown: s.points.length, total: s.total.toLocaleString() })] : []),
  ])
  const footer = [...notes.map((n) => t(`notes.${n.code}`, n.params)), ...(seriesNotes ?? [])]

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.6fr)] gap-3">
      <DataPanel
        text={text}
        onText={setText}
        source={source}
        format={format}
        onFormat={changeFormat}
        header={header}
        onHeader={changeHeader}
        summary={summary}
        error={dataError}
        busy={load.running || pasted.pending}
        onOpen={() => void openFile()}
        onSample={() => {
          setSource({ kind: 'text' })
          setText(SAMPLE)
        }}
        onCloseFile={() => setSource({ kind: 'text' })}
      />

      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
        <div className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3 shadow-card">
          <SegmentedControl<Chart>
            size="sm"
            value={config?.chart ?? 'line'}
            onValueChange={(chart) => update({ chart })}
            aria-label={t('config.chart')}
            options={(['line', 'bar', 'scatter'] as Chart[]).map((value) => ({ value, label: t(`charts.${value}`) }))}
          />
          <span className="text-xs text-fg-muted">{t('config.x')}</span>
          <Select<string>
            size="sm"
            value={config?.x == null ? 'row' : String(config.x)}
            onValueChange={(value) => update({ x: value === 'row' ? null : Number(value) })}
            disabled={!config}
            className="w-40"
            aria-label={t('config.x')}
            options={[{ value: 'row', label: t('config.rowNumber') }, ...(summary?.columns ?? []).map((c) => ({ value: String(c.index), label: c.name }))]}
          />
          {config?.chart !== 'bar' && config?.x != null && (
            <Select<XType>
              size="sm"
              value={config.xType}
              onValueChange={(xType) => update({ xType })}
              className="w-28"
              aria-label={t('config.xType')}
              options={(['auto', 'number', 'time', 'category'] as XType[]).map((value) => ({ value, label: t(`xTypes.${value}`) }))}
            />
          )}
          <span className="mx-1 h-5 w-px bg-border" />
          {config?.series.map((spec, i) => {
            const column = summary?.columns[spec.column]
            return (
              <span key={spec.column} className="flex items-center gap-1 rounded-control border border-border bg-surface-2 py-0.5 pr-0.5 pl-2 text-xs">
                <span className="size-2 rounded-full" style={{ background: PALETTE[i % PALETTE.length] }} />
                <span className="max-w-32 truncate text-fg">{column?.name}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 px-1.5"
                  title={t('config.switchAxis')}
                  onClick={() => update({ series: config.series.map((s) => (s.column === spec.column ? { ...s, axis: s.axis === 'left' ? 'right' : 'left' } : s)) })}
                >
                  <ArrowLeftRight />
                  {spec.axis === 'left' ? t('chart.leftShort') : t('chart.rightShort')}
                </Button>
                <Button size="icon-sm" variant="ghost" className="size-6" aria-label={t('config.remove')} onClick={() => update({ series: config.series.filter((s) => s.column !== spec.column) })}>
                  <X />
                </Button>
              </span>
            )
          })}
          {config && unused.length > 0 && config.series.length < MAX_SERIES && (
            <DropdownMenu
              trigger={
                <Button size="sm" variant="ghost">
                  <Plus />
                  {t('config.addSeries')}
                </Button>
              }
              items={unused.map((c) => ({ key: String(c.index), label: c.name, onSelect: () => update({ series: [...config.series, { column: c.index, axis: 'left' }] }) }))}
            />
          )}
          {config && (
            <span className="ml-auto flex items-center gap-3 text-xs text-fg-muted">
              <label className="flex items-center gap-1.5">
                {hasRight ? t('config.logLeft') : t('config.log')}
                <Switch checked={config.logLeft} onCheckedChange={(logLeft) => update({ logLeft })} aria-label={t('config.logLeft')} />
              </label>
              {hasRight && (
                <label className="flex items-center gap-1.5">
                  {t('config.logRight')}
                  <Switch checked={config.logRight} onCheckedChange={(logRight) => update({ logRight })} aria-label={t('config.logRight')} />
                </label>
              )}
            </span>
          )}
        </div>

        <Panel
          icon={<ChartLine />}
          title={t('chart.title')}
          extra={plotted.pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            plot && (
              <DropdownMenu
                trigger={
                  <Button size="sm" variant="ghost">
                    <ArrowDownToLine />
                    {t('chart.export')}
                  </Button>
                }
                items={[
                  { key: 'png', label: t('chart.savePng'), icon: <ArrowDownToLine />, onSelect: () => void exportChart('png') },
                  { key: 'svg', label: t('chart.saveSvg'), icon: <ArrowDownToLine />, onSelect: () => void exportChart('svg') },
                ]}
              />
            )
          }
          footer={
            footer.length > 0 && (
              <span className="flex min-w-0 items-center gap-2">
                <Badge variant="warning">{footer.length}</Badge>
                <span className="min-w-0 truncate" title={footer.join('\n')}>
                  {footer.join(' · ')}
                </span>
              </span>
            )
          }
          bodyClassName="min-h-0 p-2"
        >
          {!summary ? (
            <Empty icon={<ChartLine />} title={t('chart.empty')} description={t('chart.emptyHint')} />
          ) : config && config.series.length === 0 ? (
            <Empty icon={<Plus />} title={t('chart.noSeries')} description={numeric.length === 0 ? t('chart.noNumeric') : t('chart.noSeriesHint')} />
          ) : plotted.error ? (
            <Empty icon={<ChartLine />} title={errorMessage(plotted.error)} />
          ) : plot ? (
            <ChartView plot={plot} svgRef={svg} />
          ) : (
            <Empty icon={<Spinner />} title={t('chart.drawing')} />
          )}
        </Panel>
      </div>
    </div>
  )
}

export default QuickPlot
