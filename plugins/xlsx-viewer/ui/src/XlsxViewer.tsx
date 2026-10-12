import { useCallback, useEffect, useEffectEvent, useState } from 'react'
import { Badge, Button, DropdownMenu, Input, Progress, Switch, Tabs, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Download, FolderOpen, Search, Sheet, Square, Table2, Upload, X } from 'lucide-react'
import { ColumnPanel } from './ColumnPanel'
import { DataGrid } from './DataGrid'
import type { Format, Spec, WorkbookInfo } from './types'

const EXTENSIONS = ['xlsx', 'xlsm', 'xlsb', 'xls', 'ods']
const FORMATS: { value: Format; ext: string }[] = [
  { value: 'csv', ext: 'csv' },
  { value: 'tsv', ext: 'tsv' },
  { value: 'json', ext: 'json' },
  { value: 'markdown', ext: 'md' },
]
const isSpreadsheet = (path: string) => EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')
const stem = (name: string) => name.replace(/\.[^.]+$/, '')

const EMPTY_SPEC: Spec = { sort: [], query: '', filters: [] }

export function XlsxViewer() {
  const { t, errorMessage } = usePlugin()
  const openTask = useTask<WorkbookInfo>()
  const exportTask = useTask<{ rows: number; path: string }>()
  const [path, setPath] = useState<string | null>(null)
  const [header, setHeader] = useState(true)
  const [sheet, setSheet] = useState(0)
  const [search, setSearch] = useState('')
  const [spec, setSpec] = useState<Spec>(EMPTY_SPEC)
  const [selected, setSelected] = useState<number | null>(null)
  const [shown, setShown] = useState<number | null>(null)
  const [hovering, setHovering] = useState(false)

  const info = openTask.result
  const loading = openTask.running
  const current = info?.sheets[sheet]

  // 搜索框防抖后再更新视图
  useEffect(() => {
    const timer = setTimeout(() => setSpec((prev) => (prev.query === search ? prev : { ...prev, query: search })), 250)
    return () => clearTimeout(timer)
  }, [search])

  const load = useCallback(
    async (nextPath: string, withHeader: boolean) => {
      setPath(nextPath)
      setHeader(withHeader)
      setSheet(0)
      setSpec(EMPTY_SPEC)
      setSearch('')
      setSelected(null)
      await openTask.start('open', { path: nextPath, header: withHeader })
    },
    [openTask],
  )

  useEffect(() => {
    if (openTask.status === 'failed' && openTask.error?.code !== 'task.cancelled' && openTask.error) toast.error(errorMessage(openTask.error))
  }, [openTask.status, openTask.error, errorMessage])

  useEffect(() => {
    if (exportTask.status === 'succeeded' && exportTask.result) toast.success(t('export.done', { count: exportTask.result.rows }))
    if (exportTask.status === 'failed' && exportTask.error && exportTask.error.code !== 'task.cancelled') toast.error(errorMessage(exportTask.error))
  }, [exportTask.status, exportTask.result, exportTask.error, errorMessage, t])

  const onDrop = useEffectEvent((paths: string[]) => {
    const next = paths.find(isSpreadsheet)
    if (next) load(next, header)
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: onDrop, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [header])

  const pick = async () => {
    try {
      const next = await host.dialog.openFile([{ name: t('open.filter'), extensions: EXTENSIONS }])
      if (next) await load(next, header)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const close = () => {
    setPath(null)
    openTask.reset()
    exportTask.reset()
  }

  const toggleHeader = async () => {
    if (path) await load(path, !header)
  }

  const switchSheet = (value: string) => {
    setSheet(Number(value))
    setSpec(EMPTY_SPEC)
    setSearch('')
    setSelected(null)
  }

  const columnName = useCallback(
    (column: number) => current?.columns[column]?.name ?? t('table.column', { n: column + 1 }),
    [current, t],
  )

  const toggleSort = (column: number) =>
    setSpec((prev) => {
      const existing = prev.sort.find((s) => s.column === column)
      const sort = !existing ? [{ column, desc: false }] : existing.desc ? [] : [{ column, desc: true }]
      return { ...prev, sort }
    })

  const addFilter = (filter: import('./types').Filter) => setSpec((prev) => ({ ...prev, filters: [...prev.filters, filter] }))
  const removeFilter = (index: number) => setSpec((prev) => ({ ...prev, filters: prev.filters.filter((_, i) => i !== index) }))

  const exportAs = async (format: Format, ext: string) => {
    if (!info) return
    try {
      const name = info.name ?? 'workbook'
      const sheetName = info.sheets[sheet]?.name ?? `sheet${sheet + 1}`
      const target = await host.dialog.saveFile(`${stem(name)}-${sheetName}.${ext}`, [
        { name: format.toUpperCase(), extensions: [ext] },
      ])
      if (target) await exportTask.start('export', { id: info.id, sheet, path: target, format, ...spec })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  if (!info) {
    const progress = openTask.progress && openTask.progress.total > 0 ? (openTask.progress.done / openTask.progress.total) * 100 : null
    return (
      <div className="flex h-full flex-col gap-3">
        <button
          type="button"
          onClick={pick}
          disabled={loading}
          className={cn(
            'flex min-h-0 flex-1 flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          {loading ? (
            <div className="w-72 space-y-2">
              <Progress value={progress} />
              <p className="text-[13px]">{openTask.stage ? t(`stages.${openTask.stage}`) : t('stages.xlsx.read')}</p>
            </div>
          ) : (
            <>
              <Upload className="size-6" />
              <span className="text-[13px] font-medium">{t('open.drop')}</span>
              <span className="text-xs text-fg-subtle">{t('open.hint')}</span>
            </>
          )}
        </button>
        <div className="flex justify-center">
          {loading && (
            <Button onClick={openTask.cancel}>
              <Square />
              {t('open.cancel')}
            </Button>
          )}
        </div>
      </div>
    )
  }

  const active = info.sheets[sheet] ?? info.sheets[0]
  const filtered = spec.query.trim() !== '' || spec.filters.length > 0
  const exportProgress = exportTask.progress && exportTask.progress.total > 0 ? (exportTask.progress.done / exportTask.progress.total) * 100 : null

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <Table2 className="size-4 shrink-0 text-fg-subtle" />
        <span className="max-w-60 truncate text-[13px] font-semibold text-fg" title={info.path ?? undefined}>
          {info.name ?? t('open.workbook')}
        </span>
        <Badge>{t('table.sheets', { count: info.sheets.length })}</Badge>
        {info.path && <Badge>{formatBytes(info.bytes)}</Badge>}
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            <Switch size="sm" checked={header} onCheckedChange={toggleHeader} aria-label={t('options.header')} />
            {t('options.header')}
          </label>
          <span className="mx-1 h-5 w-px bg-border" />
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={exportTask.running || !active}>
                <Download />
                {t('export.title')}
              </Button>
            }
            items={FORMATS.map((f) => ({ key: f.value, label: t(`export.${f.value}`), onSelect: () => exportAs(f.value, f.ext) }))}
          />
          <Button size="sm" onClick={pick}>
            <FolderOpen />
            {t('open.another')}
          </Button>
          <Button size="icon-sm" variant="ghost" aria-label={t('open.close')} onClick={close}>
            <X />
          </Button>
        </div>
      </section>

      <div className="flex flex-wrap items-center gap-2">
        <Tabs
          value={String(sheet)}
          onValueChange={switchSheet}
          aria-label={t('table.sheetsNav')}
          className="max-w-full overflow-x-auto"
          items={info.sheets.map((s, index) => ({
            value: String(index),
            label: (
              <span className="flex max-w-40 items-center gap-1.5">
                <Sheet className="size-3.5 shrink-0 text-fg-subtle" />
                <span className="truncate" title={s.name}>
                  {s.name}
                </span>
              </span>
            ),
          }))}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Input
          size="sm"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t('table.search')}
          aria-label={t('table.search')}
          leading={<Search />}
          wrapperClassName="w-72"
        />
        {spec.filters.map((filter, index) => (
          <Badge key={index} variant="info" className="gap-1 pr-1">
            <span className="max-w-72 truncate">
              {columnName(filter.column)} {t(`filters.ops.${filter.op}`)} {filter.value}
            </span>
            <button
              type="button"
              aria-label={t('filters.remove')}
              onClick={() => removeFilter(index)}
              className="rounded-sm p-0.5 outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
            >
              <X className="size-3" />
            </button>
          </Badge>
        ))}
        {spec.filters.length > 1 && (
          <Button size="sm" variant="ghost" onClick={() => setSpec((prev) => ({ ...prev, filters: [] }))}>
            {t('filters.clear')}
          </Button>
        )}
        {exportTask.running && (
          <div className="ml-auto flex w-56 items-center gap-2">
            <Progress value={exportProgress} className="flex-1" />
            <Button size="icon-sm" variant="ghost" aria-label={t('open.cancel')} onClick={exportTask.cancel}>
              <Square />
            </Button>
          </div>
        )}
      </div>

      <div className={cn('grid min-h-0 flex-1 gap-3', selected !== null ? 'grid-cols-[minmax(0,1fr)_280px]' : 'grid-cols-1')}>
        <div className={cn('min-h-0 overflow-hidden rounded-card border border-border bg-surface shadow-card', hovering && 'ring-2 ring-primary')}>
          {active ? (
            <DataGrid
              key={`${info.id}-${sheet}`}
              info={info}
              sheet={sheet}
              spec={spec}
              selected={selected}
              onSort={toggleSort}
              onSelect={(c) => setSelected(c === selected ? null : c)}
              onTotal={setShown}
              columnName={columnName}
            />
          ) : null}
        </div>
        {selected !== null && active && selected < active.columns.length && (
          <ColumnPanel key={`${info.id}-${sheet}-${selected}`} info={info} sheet={sheet} spec={spec} column={selected} name={columnName(selected)} onAddFilter={addFilter} onClose={() => setSelected(null)} />
        )}
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-1 text-xs text-fg-muted tabular-nums">
        {active && (
          <span>
            {filtered && shown !== null
              ? t('table.filtered', { shown: shown.toLocaleString(), total: active.rows.toLocaleString() })
              : t('table.rows', { count: active.rows, formatted: active.rows.toLocaleString() })}
          </span>
        )}
        {active && <span>{t('table.columns', { count: active.columns.length })}</span>}
        <span className="ml-auto">{t('table.loaded', { ms: info.elapsedMs })}</span>
      </div>
    </div>
  )
}
