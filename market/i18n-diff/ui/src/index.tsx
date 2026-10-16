import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { Badge, Button, DropdownMenu, Empty, Input, Modal, Panel, SegmentedControl, Select, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, Download, FilePlus2, Languages, RefreshCw, Save, Star, X } from 'lucide-react'
import { EXTENSIONS, ISSUES, baseName, type Cell, type Comparison, type Fill, type Issue, type LoadedFile, type Row, type WriteResult } from './types'

const MAX_FILES = 20
/** 表格最多渲染的行数；报告导出包含全部 */
const MAX_VISIBLE = 2000

const ISSUE_VARIANT: Record<Issue, 'danger' | 'warning' | 'info' | 'neutral'> = {
  missing: 'danger',
  extra: 'info',
  empty: 'warning',
  untranslated: 'warning',
  placeholders: 'danger',
  duplicate: 'neutral',
}

type Filter = 'issues' | 'all'
type View = 'flat' | 'grouped'

/** 同名文件（不同目录下的 en.json）用「目录/文件名」区分 */
function displayNames(files: LoadedFile[]) {
  return files.map((file) => {
    const same = files.filter((other) => other.name === file.name).length > 1
    if (!same) return file.name
    const parts = file.path.split(/[\\/]/)
    return parts.slice(-2).join('/')
  })
}

function CellView({ cell, editing, value, onEdit }: { cell: Cell; editing: boolean; value: string | undefined; onEdit: (value: string) => void }) {
  const { t } = usePlugin()
  const current = value ?? cell.value ?? ''
  return (
    <div className="min-w-0 space-y-1">
      {editing && cell.issues[0] !== 'extra' ? (
        <Input
          size="sm"
          className={cn('w-full', value !== undefined && 'border-primary')}
          value={current}
          placeholder={cell.value === null ? t('table.missingValue') : ''}
          aria-label={t('table.edit')}
          onChange={(event) => onEdit(event.target.value)}
        />
      ) : (
        <p className={cn('truncate', cell.value === null ? 'text-fg-subtle' : 'text-fg')} title={cell.value ?? undefined} data-selectable>
          {cell.value === null ? '—' : cell.value || <span className="text-fg-subtle">{t('table.emptyValue')}</span>}
        </p>
      )}
      {cell.issues.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {cell.issues.map((issue) => (
            <Badge key={issue} variant={ISSUE_VARIANT[issue]}>
              {t(`issue.${issue}`)}
            </Badge>
          ))}
          {cell.missingPlaceholders && cell.missingPlaceholders.length > 0 && (
            <span className="text-[11px] text-danger">{t('table.lacks', { list: cell.missingPlaceholders.join(' ') })}</span>
          )}
          {cell.extraPlaceholders && cell.extraPlaceholders.length > 0 && (
            <span className="text-[11px] text-danger">{t('table.adds', { list: cell.extraPlaceholders.join(' ') })}</span>
          )}
        </div>
      )}
    </div>
  )
}

function I18nDiff() {
  const { t, call, errorMessage } = usePlugin()
  const [files, setFiles] = useState<LoadedFile[]>([])
  const [filter, setFilter] = useState<Filter>('issues')
  const [issueFilter, setIssueFilter] = useState<Issue | 'any'>('any')
  const [view, setView] = useState<View>('flat')
  const [target, setTarget] = useState<string | null>(null)
  const [edits, setEdits] = useState<Record<string, string>>({})
  const [fill, setFill] = useState<Fill>('none')
  const [preview, setPreview] = useState<WriteResult | null>(null)
  const [busy, setBusy] = useState(false)
  const [dropping, setDropping] = useState(false)

  const names = useMemo(() => displayNames(files), [files])
  const payload = useMemo(() => files.map((file, index) => ({ name: names[index], text: file.text })), [files, names])
  const ready = files.length >= 2
  const { result, error, pending } = useDebouncedCall<Comparison>(ready ? 'compare' : null, ready ? { files: payload } : null, [payload])

  const load = async (paths: string[]) => {
    const fresh = paths.filter((path) => !files.some((file) => file.path === path))
    if (files.length + fresh.length > MAX_FILES) toast.info(t('files.tooMany', { max: MAX_FILES }))
    const loaded: LoadedFile[] = []
    for (const path of fresh.slice(0, MAX_FILES - files.length)) {
      try {
        loaded.push({ path, name: baseName(path), text: await host.fs.readText(path) })
      } catch (reason) {
        toast.error(errorMessage(reason))
      }
    }
    if (loaded.length > 0) setFiles((prev) => [...prev, ...loaded])
  }

  const pick = async () => {
    try {
      const paths = await host.dialog.openFiles([{ name: t('files.filter'), extensions: EXTENSIONS }])
      if (paths.length > 0) await load(paths)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => void loadRef.current(paths), over: setDropping })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const reload = async () => {
    try {
      const next = await Promise.all(files.map(async (file) => ({ ...file, text: await host.fs.readText(file.path) })))
      setFiles(next)
      setEdits({})
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const remove = (path: string) => {
    setFiles((prev) => prev.filter((file) => file.path !== path))
    if (target === path) {
      setTarget(null)
      setEdits({})
    }
  }

  const makeBase = (path: string) => {
    setFiles((prev) => [...prev.filter((file) => file.path === path), ...prev.filter((file) => file.path !== path)])
    if (target === path) {
      setTarget(null)
      setEdits({})
    }
  }

  const targetIndex = target ? files.findIndex((file) => file.path === target) : -1
  const writableTargets = result
    ? files.flatMap((file, index) => (index > 0 && result.files[index]?.writable ? [{ value: file.path, label: names[index] }] : []))
    : []

  const rows = useMemo(() => {
    if (!result) return [] as Row[]
    return result.rows.filter((row) => {
      if (filter === 'all') return true
      const cells = targetIndex > 0 ? [row.cells[targetIndex]] : row.cells
      return cells.some((cell) => (issueFilter === 'any' ? cell.issues.length > 0 : cell.issues.includes(issueFilter)))
    })
  }, [result, filter, issueFilter, targetIndex])

  const groups = useMemo(() => {
    if (view === 'flat') return [{ name: '', rows }]
    const map = new Map<string, Row[]>()
    for (const row of rows) {
      const head = row.key.split('.')[0]
      map.set(head, [...(map.get(head) ?? []), row])
    }
    return [...map.entries()].map(([name, list]) => ({ name, rows: list }))
  }, [rows, view])

  const edited = Object.keys(edits).length
  const writeArgs = () => ({
    base: { name: names[0], text: files[0].text },
    target: { name: names[targetIndex], text: files[targetIndex].text },
    edits,
    fill,
  })

  const openPreview = async () => {
    if (targetIndex <= 0) return
    setBusy(true)
    try {
      setPreview(await call<WriteResult>('write', writeArgs()))
    } catch (reason) {
      toast.error(errorMessage(reason))
    } finally {
      setBusy(false)
    }
  }

  const save = async () => {
    if (!preview || targetIndex <= 0) return
    setBusy(true)
    try {
      const file = files[targetIndex]
      await host.fs.writeText(file.path, preview.text)
      setFiles((prev) => prev.map((item) => (item.path === file.path ? { ...item, text: preview.text } : item)))
      setEdits({})
      setPreview(null)
      toast.success(t('write.saved', { file: names[targetIndex] }))
    } catch (reason) {
      toast.error(errorMessage(reason))
    } finally {
      setBusy(false)
    }
  }

  const exportReport = async (format: 'csv' | 'json', mode: 'copy' | 'save') => {
    try {
      const text = await call<string>('report', { files: payload, format })
      if (mode === 'copy') {
        await host.clipboard.writeText(text)
        toast.success(t('common:copied'))
        return
      }
      const path = await host.dialog.saveFile(`i18n-report.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  if (files.length === 0) {
    return (
      <Panel className="h-full" bodyClassName="flex h-full items-center justify-center p-3">
        <button
          type="button"
          onClick={pick}
          className={cn(
            'flex h-full min-h-64 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            dropping ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          <Languages className="size-6" />
          <span className="text-[13px] font-medium">{t('files.drop')}</span>
          <span className="max-w-md text-xs text-fg-subtle">{t('files.dropHint')}</span>
        </button>
      </Panel>
    )
  }

  return (
    <div className={cn('grid h-full min-h-0 grid-cols-[240px_minmax(0,1fr)] gap-3', dropping && 'rounded-card ring-2 ring-primary')}>
      <Panel
        icon={<Languages />}
        title={t('files.title')}
        actions={
          <>
            <Tooltip content={t('files.reload')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('files.reload')} onClick={() => void reload()}>
                <RefreshCw />
              </Button>
            </Tooltip>
            <Tooltip content={t('files.add')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('files.add')} onClick={() => void pick()} disabled={files.length >= MAX_FILES}>
                <FilePlus2 />
              </Button>
            </Tooltip>
          </>
        }
        bodyClassName="space-y-2 overflow-auto p-2"
      >
        {files.map((file, index) => {
          const summary = result?.files[index]
          return (
            <div key={file.path} className={cn('rounded-control border px-2.5 py-2', index === 0 ? 'border-primary bg-primary-soft' : 'border-border')}>
              <div className="flex items-center gap-1">
                {index === 0 && <Star className="size-3.5 shrink-0 text-primary" aria-hidden />}
                <p className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg" title={file.path}>
                  {names[index]}
                </p>
                {index > 0 && (
                  <Tooltip content={t('files.makeBase')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('files.makeBase')} onClick={() => makeBase(file.path)}>
                      <Star />
                    </Button>
                  </Tooltip>
                )}
                <Tooltip content={t('files.remove')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('files.remove')} onClick={() => remove(file.path)}>
                    <X />
                  </Button>
                </Tooltip>
              </div>
              {summary && (
                <div className="mt-1 space-y-1 text-[11px] text-fg-muted">
                  <p className="tabular-nums">{index === 0 ? t('files.baseSummary', { keys: summary.keys }) : t('files.summary', { keys: summary.keys, coverage: summary.coverage })}</p>
                  {index > 0 && (
                    <div className="flex flex-wrap gap-1">
                      {ISSUES.filter((issue) => summary[issue] > 0).map((issue) => (
                        <Badge key={issue} variant={ISSUE_VARIANT[issue]}>
                          {t(`issue.${issue}`)} {summary[issue]}
                        </Badge>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )
        })}
        {files.length < 2 && <p className="px-1 text-xs text-fg-subtle">{t('files.needTwo')}</p>}
      </Panel>

      <Panel
        icon={<Languages />}
        title={t('table.title')}
        extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
        actions={
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={!result}>
                <Download />
                {t('export.title')}
                <ChevronDown />
              </Button>
            }
            items={[
              { key: 'copy-csv', label: t('export.copyCsv'), onSelect: () => void exportReport('csv', 'copy') },
              { key: 'save-csv', label: t('export.saveCsv'), onSelect: () => void exportReport('csv', 'save') },
              { key: 'copy-json', label: t('export.copyJson'), onSelect: () => void exportReport('json', 'copy') },
              { key: 'save-json', label: t('export.saveJson'), onSelect: () => void exportReport('json', 'save') },
            ]}
          />
        }
        footer={
          result && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-xs text-fg-muted">{t('write.target')}</span>
              <Select<string>
                size="sm"
                className="w-48"
                value={target}
                placeholder={writableTargets.length > 0 ? t('write.pick') : t('write.none')}
                disabled={writableTargets.length === 0}
                onValueChange={(value) => {
                  setTarget(value)
                  setEdits({})
                }}
                aria-label={t('write.target')}
                options={writableTargets}
              />
              <Select<Fill>
                size="sm"
                value={fill}
                onValueChange={setFill}
                aria-label={t('write.fill')}
                options={[
                  { value: 'none', label: t('write.fillNone') },
                  { value: 'empty', label: t('write.fillEmpty') },
                  { value: 'base', label: t('write.fillBase') },
                ]}
              />
              {edited > 0 && <span className="text-xs text-primary">{t('write.edited', { count: edited })}</span>}
              <Button size="sm" variant="primary" className="ml-auto" disabled={targetIndex <= 0 || busy || (edited === 0 && fill === 'none')} onClick={() => void openPreview()}>
                <Save />
                {t('write.preview')}
              </Button>
            </div>
          )
        }
        bodyClassName="flex min-h-0 flex-col gap-3 overflow-auto p-3"
      >
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <Empty icon={<Languages />} title={t('files.needTwo')} description={t('files.dropHint')} />
        ) : (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <SegmentedControl<Filter>
                size="sm"
                value={filter}
                onValueChange={setFilter}
                aria-label={t('table.filter')}
                options={[
                  { value: 'issues', label: t('table.onlyIssues') },
                  { value: 'all', label: t('table.all', { count: result.rows.length }) },
                ]}
              />
              {filter === 'issues' && (
                <Select<Issue | 'any'>
                  size="sm"
                  value={issueFilter}
                  onValueChange={setIssueFilter}
                  aria-label={t('table.issueType')}
                  options={[{ value: 'any', label: t('table.anyIssue') }, ...ISSUES.map((issue) => ({ value: issue, label: t(`issue.${issue}`) }))]}
                />
              )}
              <SegmentedControl<View>
                size="sm"
                className="ml-auto"
                value={view}
                onValueChange={setView}
                aria-label={t('table.view')}
                options={[
                  { value: 'flat', label: t('table.flat') },
                  { value: 'grouped', label: t('table.grouped') },
                ]}
              />
            </div>
            {rows.length === 0 ? (
              <Empty icon={<Languages />} title={t('table.clean')} />
            ) : (
              <div className="overflow-auto rounded-control border border-border">
                <table className="w-full table-fixed border-collapse text-xs">
                  <thead className="sticky top-0 z-10 bg-surface-2 text-fg-muted">
                    <tr>
                      <th className="w-56 px-2 py-1.5 text-left font-medium">{t('table.key')}</th>
                      {names.map((name, index) => (
                        <th key={files[index].path} className={cn('px-2 py-1.5 text-left font-medium', index === targetIndex && 'text-primary')}>
                          {name}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {groups.map((group) => (
                      <GroupRows key={group.name} name={group.name} columns={names.length + 1}>
                        {group.rows.slice(0, MAX_VISIBLE).map((row) => (
                          <tr key={row.key} className="border-t border-border align-top">
                            <td className="px-2 py-1.5">
                              <p className="truncate font-mono text-fg" title={row.key} data-selectable>
                                {view === 'grouped' ? row.key.slice(group.name.length + 1) || row.key : row.key}
                              </p>
                            </td>
                            {row.cells.map((cell, index) => (
                              <td key={files[index].path} className="px-2 py-1.5">
                                <CellView
                                  cell={cell}
                                  editing={index === targetIndex}
                                  value={index === targetIndex ? edits[row.key] : undefined}
                                  onEdit={(value) => setEdits((prev) => ({ ...prev, [row.key]: value }))}
                                />
                              </td>
                            ))}
                          </tr>
                        ))}
                      </GroupRows>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
            {rows.length > MAX_VISIBLE && <p className="text-xs text-fg-subtle">{t('table.more', { count: MAX_VISIBLE })}</p>}
          </>
        )}
      </Panel>

      <Modal
        open={preview !== null}
        onOpenChange={(open) => !open && setPreview(null)}
        size="lg"
        title={t('write.previewTitle', { file: targetIndex > 0 ? names[targetIndex] : '' })}
        description={preview ? t('write.changes', { added: preview.added.length, updated: preview.updated.length }) : undefined}
        closeLabel={t('write.cancel')}
        footer={
          <>
            <Button onClick={() => setPreview(null)}>{t('write.cancel')}</Button>
            <Button variant="primary" loading={busy} disabled={!preview || preview.added.length + preview.updated.length === 0} onClick={() => void save()}>
              <Save />
              {t('write.save')}
            </Button>
          </>
        }
      >
        {preview && (
          <pre className="max-h-[60vh] overflow-auto rounded-control border border-border bg-surface-2 p-3 font-mono text-xs text-fg" data-selectable>
            {preview.text}
          </pre>
        )}
      </Modal>
    </div>
  )
}

function GroupRows({ name, columns, children }: { name: string; columns: number; children: ReactNode }) {
  return (
    <>
      {name && (
        <tr className="border-t border-border bg-surface-2">
          <td colSpan={columns} className="px-2 py-1 font-mono text-[11px] font-semibold text-fg-muted">
            {name}
          </td>
        </tr>
      )}
      {children}
    </>
  )
}

export default I18nDiff
