import { useCallback, useEffect, useEffectEvent, useState } from 'react'
import { Badge, Button, DropdownMenu, Input, Progress, Select, Switch, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ClipboardPaste, Download, FolderOpen, Search, Square, Table2, Upload, X } from 'lucide-react'
import { ColumnPanel } from './ColumnPanel'
import { DataGrid } from './DataGrid'
import type { Filter, Format, Spec, TableInfo } from './types'

const EXTENSIONS = ['csv', 'tsv', 'txt', 'tab', 'psv']
const DELIMITERS = ['auto', ',', '\\t', ';', '|'] as const
const ENCODINGS = ['auto', 'UTF-8', 'GB18030', 'UTF-16LE', 'Big5', 'Shift_JIS', 'windows-1252'] as const
const FORMATS: { value: Format; ext: string }[] = [
  { value: 'csv', ext: 'csv' },
  { value: 'tsv', ext: 'tsv' },
  { value: 'json', ext: 'json' },
  { value: 'markdown', ext: 'md' },
]
const isTable = (path: string) => EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')
const stem = (name: string) => name.replace(/\.[^.]+$/, '')

type Source = { kind: 'file'; path: string } | { kind: 'text'; text: string }

interface Options {
  delimiter: string
  encoding: string
  header: boolean | null
}

const AUTO: Options = { delimiter: 'auto', encoding: 'auto', header: null }
const EMPTY_SPEC: Spec = { sort: [], query: '', filters: [] }

export function CsvViewer() {
  const { t, call, errorMessage } = usePlugin()
  const openTask = useTask<TableInfo>()
  const exportTask = useTask<{ rows: number; path: string }>()
  const [source, setSource] = useState<Source | null>(null)
  const [pasted, setPasted] = useState<TableInfo | null>(null)
  const [options, setOptions] = useState<Options>(AUTO)
  const [search, setSearch] = useState('')
  const [spec, setSpec] = useState<Spec>(EMPTY_SPEC)
  const [selected, setSelected] = useState<number | null>(null)
  const [shown, setShown] = useState<number | null>(null)
  const [hovering, setHovering] = useState(false)

  const info = source?.kind === 'text' ? pasted : openTask.result
  const loading = openTask.running

  // 搜索框防抖后再更新视图
  useEffect(() => {
    const timer = setTimeout(() => setSpec((prev) => (prev.query === search ? prev : { ...prev, query: search })), 250)
    return () => clearTimeout(timer)
  }, [search])

  const load = async (next: Source, nextOptions: Options) => {
    const args = {
      delimiter: nextOptions.delimiter === 'auto' ? null : nextOptions.delimiter,
      encoding: nextOptions.encoding === 'auto' ? null : nextOptions.encoding,
      header: nextOptions.header,
    }
    setSource(next)
    setOptions(nextOptions)
    setSpec(EMPTY_SPEC)
    setSearch('')
    setSelected(null)
    if (next.kind === 'file') {
      setPasted(null)
      await openTask.start('open', { path: next.path, ...args })
    } else {
      openTask.reset()
      try {
        setPasted(await call<TableInfo>('parse_text', { text: next.text, ...args }))
      } catch (error) {
        setPasted(null)
        setSource(null)
        toast.error(errorMessage(error))
      }
    }
  }

  useEffect(() => {
    if (openTask.status === 'failed' && openTask.error?.code !== 'task.cancelled' && openTask.error) toast.error(errorMessage(openTask.error))
  }, [openTask.status, openTask.error, errorMessage])

  useEffect(() => {
    if (exportTask.status === 'succeeded' && exportTask.result) toast.success(t('export.done', { count: exportTask.result.rows }))
    if (exportTask.status === 'failed' && exportTask.error && exportTask.error.code !== 'task.cancelled') toast.error(errorMessage(exportTask.error))
  }, [exportTask.status, exportTask.result, exportTask.error, errorMessage, t])

  const onDrop = useEffectEvent((paths: string[]) => {
    const path = paths.find(isTable)
    if (path) load({ kind: 'file', path }, AUTO)
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: onDrop, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: EXTENSIONS }])
      if (path) await load({ kind: 'file', path }, AUTO)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const paste = async () => {
    try {
      const text = await host.clipboard.readText()
      if (text?.trim()) await load({ kind: 'text', text }, AUTO)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const reload = (patch: Partial<Options>) => source && load(source, { ...options, ...patch })

  const close = () => {
    setSource(null)
    setPasted(null)
    openTask.reset()
  }

  const columnName = useCallback(
    (column: number) => info?.columns[column]?.name ?? t('table.column', { n: column + 1 }),
    [info, t],
  )

  const toggleSort = (column: number) =>
    setSpec((prev) => {
      const current = prev.sort.find((s) => s.column === column)
      const sort = !current ? [{ column, desc: false }] : current.desc ? [] : [{ column, desc: true }]
      return { ...prev, sort }
    })

  const addFilter = (filter: Filter) => setSpec((prev) => ({ ...prev, filters: [...prev.filters, filter] }))
  const removeFilter = (index: number) => setSpec((prev) => ({ ...prev, filters: prev.filters.filter((_, i) => i !== index) }))

  const exportAs = async (format: Format, ext: string) => {
    if (!info) return
    try {
      const path = await host.dialog.saveFile(`${stem(info.name ?? 'table')}-export.${ext}`, [{ name: format.toUpperCase(), extensions: [ext] }])
      if (path) await exportTask.start('export', { id: info.id, path, format, ...spec })
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
              <p className="text-[13px]">{openTask.stage ? t(`stages.${openTask.stage}`) : t('stages.csv.read')}</p>
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
          {loading ? (
            <Button onClick={openTask.cancel}>
              <Square />
              {t('open.cancel')}
            </Button>
          ) : (
            <Button onClick={paste}>
              <ClipboardPaste />
              {t('open.paste')}
            </Button>
          )}
        </div>
      </div>
    )
  }

  const filtered = spec.query.trim() !== '' || spec.filters.length > 0
  const exportProgress = exportTask.progress && exportTask.progress.total > 0 ? (exportTask.progress.done / exportTask.progress.total) * 100 : null

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <Table2 className="size-4 shrink-0 text-fg-subtle" />
        <span className="max-w-60 truncate text-[13px] font-semibold text-fg" title={info.path ?? undefined}>
          {info.name ?? t('open.pasted')}
        </span>
        <Badge>{t('table.columns', { count: info.columns.length })}</Badge>
        {info.path && <Badge>{formatBytes(info.bytes)}</Badge>}
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <Select<string>
            size="sm"
            value={options.delimiter}
            onValueChange={(delimiter) => reload({ delimiter })}
            aria-label={t('options.delimiter')}
            options={DELIMITERS.map((d) => ({
              value: d,
              label:
                d === 'auto'
                  ? `${t('options.delimiter')}: ${t('options.auto')} (${info.delimiter === '\\t' ? t('options.tab') : info.delimiter})`
                  : `${t('options.delimiter')}: ${{ ',': t('options.comma'), '\\t': t('options.tab'), ';': t('options.semicolon'), '|': t('options.pipe') }[d]}`,
            }))}
          />
          <Select<string>
            size="sm"
            value={options.encoding}
            onValueChange={(encoding) => reload({ encoding })}
            aria-label={t('options.encoding')}
            options={ENCODINGS.map((e) => ({ value: e, label: e === 'auto' ? `${t('options.auto')} (${info.encoding})` : e }))}
          />
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            <Switch size="sm" checked={info.hasHeader} onCheckedChange={(header) => reload({ header })} aria-label={t('options.header')} />
            {t('options.header')}
          </label>
          <span className="mx-1 h-5 w-px bg-border" />
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={exportTask.running}>
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
          <DataGrid info={info} spec={spec} selected={selected} onSort={toggleSort} onSelect={(c) => setSelected(c === selected ? null : c)} onTotal={setShown} columnName={columnName} />
        </div>
        {selected !== null && selected < info.columns.length && (
          <ColumnPanel key={`${info.id}-${selected}`} info={info} spec={spec} column={selected} name={columnName(selected)} onAddFilter={addFilter} onClose={() => setSelected(null)} />
        )}
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-1 text-xs text-fg-muted tabular-nums">
        <span>
          {filtered && shown !== null
            ? t('table.filtered', { shown: shown.toLocaleString(), total: info.rows.toLocaleString() })
            : t('table.rows', { count: info.rows, formatted: info.rows.toLocaleString() })}
        </span>
        <span>{info.encoding}</span>
        <span className="ml-auto">{t('table.loaded', { ms: info.elapsedMs })}</span>
      </div>
    </div>
  )
}
