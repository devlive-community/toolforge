import { useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, Empty, Panel, SegmentedControl, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { AlertTriangle, FilePlus2, FileText, FolderOpen, FolderPlus, FolderSearch, Languages, RefreshCw, Upload, X } from 'lucide-react'
import { UNICODE, type Converted, type FileInfo, type Item, type NewLine, type Output, type Preview, type Scanned } from './types'

const ROW_HEIGHT = 44
const STATUS_BADGE = { converted: 'success', unchanged: 'neutral', failed: 'danger' } as const

function TextEncoding() {
  const { t, call, errorMessage } = usePlugin()
  const [to, setTo] = usePluginState('to', 'UTF-8')
  const [bom, setBom] = usePluginState('bom', false)
  const [newline, setNewline] = usePluginState<NewLine>('newline', 'keep')
  const [output, setOutput] = usePluginState<Output>('output', 'inplace')
  const [backup, setBackup] = usePluginState('backup', true)
  const [outputDir, setOutputDir] = useState<string | null>(null)
  const [encodings, setEncodings] = useState<string[]>(['UTF-8'])
  const [files, setFiles] = useState<FileInfo[]>([])
  const [overrides, setOverrides] = useState<Record<string, string>>({})
  const [results, setResults] = useState<Record<string, Item>>({})
  const [selected, setSelected] = useState<string | null>(null)
  const [recursive, setRecursive] = useState(true)
  const [hovering, setHovering] = useState(false)
  const task = useTask<Converted>()
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    call<string[]>('encodings', {}).then(setEncodings, () => {})
  }, [call])

  const sourceOf = (file: FileInfo) => overrides[file.path] ?? file.encoding
  const encodingOptions = encodings.map((value) => ({ value, label: t(`encodings.${value}`) }))

  const scan = async (paths: string[], replace = false) => {
    if (paths.length === 0) return
    try {
      const scanned = await call<Scanned>('scan', { paths, recursive })
      setFiles((prev) => {
        if (replace) {
          const fresh = new Map(scanned.files.map((f) => [f.path, f]))
          return prev.map((f) => fresh.get(f.path) ?? f)
        }
        const known = new Set(prev.map((f) => f.path))
        return [...prev, ...scanned.files.filter((f) => !known.has(f.path))]
      })
      if (!replace) {
        setSelected((prev) => prev ?? scanned.files.find((f) => !f.error)?.path ?? null)
        if (scanned.skipped.binary > 0) toast.info(t('files.skippedBinary', { count: scanned.skipped.binary }))
        if (scanned.skipped.tooMany > 0) toast.info(t('files.skippedTooMany', { count: scanned.skipped.tooMany }))
        if (scanned.files.length === 0 && scanned.skipped.binary === 0) toast.info(t('files.noneFound'))
      }
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const scanRef = useRef(scan)
  useEffect(() => {
    scanRef.current = scan
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => void scanRef.current(paths), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  // 转换完成：记录每个文件的结果；覆盖原文件时重新识别它们
  const [handled, setHandled] = useState<unknown>(null)
  if (task.status === 'succeeded' && task.result && task.result !== handled) {
    setHandled(task.result)
    setResults(Object.fromEntries(task.result.items.map((item) => [item.path, item])))
  }
  useEffect(() => {
    if (task.status === 'succeeded' && task.result) {
      const { summary, items } = task.result
      const message = t('convert.done', summary)
      if (summary.failed > 0) toast.error(message)
      else toast.success(message)
      if (output === 'inplace') {
        const changed = items.filter((i) => i.status === 'converted').map((i) => i.path)
        setOverrides((prev) => Object.fromEntries(Object.entries(prev).filter(([path]) => !changed.includes(path))))
        void scanRef.current(changed, true)
      }
    }
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pickFiles = async () => {
    try {
      await scan(await host.dialog.openFiles())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickFolder = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) await scan([dir])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickOutput = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const remove = (path: string) => {
    setFiles((prev) => prev.filter((f) => f.path !== path))
    if (selected === path) setSelected(null)
  }
  const clear = () => {
    setFiles([])
    setOverrides({})
    setResults({})
    setSelected(null)
  }

  const current = files.find((f) => f.path === selected) ?? null
  const currentSource = current ? sourceOf(current) : null
  const previewArgs = current && currentSource ? { path: current.path, encoding: currentSource, target: to } : null
  const preview = useDebouncedCall<Preview>(previewArgs ? 'preview' : null, previewArgs, [current?.path, currentSource, to, handled])

  const convertible = files.filter((f) => !f.error && sourceOf(f))
  const counts = convertible.reduce<Record<string, number>>((acc, f) => {
    const key = f.ascii && !overrides[f.path] ? 'ASCII' : sourceOf(f)!
    acc[key] = (acc[key] ?? 0) + 1
    return acc
  }, {})
  const bomAllowed = UNICODE.includes(to)
  const start = () =>
    task.start('convert', {
      files: convertible.map((f) => ({ path: f.path, encoding: sourceOf(f) })),
      to,
      bom: bomAllowed && bom,
      newline,
      outputDir: output === 'folder' ? outputDir : null,
      backup,
    })

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: files.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  })

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_300px] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<FileText />}
          title={t('files.title')}
          extra={files.length > 0 && <Badge>{files.length}</Badge>}
          className="min-h-0 flex-[3]"
          actions={
            <>
              <Button size="sm" onClick={pickFiles} disabled={task.running}>
                <FilePlus2 />
                {t('files.add')}
              </Button>
              <Button size="sm" onClick={pickFolder} disabled={task.running}>
                <FolderPlus />
                {t('files.addFolder')}
              </Button>
              <Tooltip content={t('files.recursiveHint')}>
                <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                  <Switch size="sm" checked={recursive} onCheckedChange={setRecursive} aria-label={t('files.recursive')} />
                  {t('files.recursive')}
                </label>
              </Tooltip>
              <Button size="icon-sm" variant="ghost" aria-label={t('common:clear')} disabled={task.running || files.length === 0} onClick={clear}>
                <X />
              </Button>
            </>
          }
          bodyClassName="p-0"
        >
          {files.length === 0 ? (
            <div className="h-full p-3">
              <button
                type="button"
                onClick={pickFolder}
                className={cn(
                  'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                  'focus-visible:ring-2 focus-visible:ring-ring',
                  hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
                )}
              >
                <Upload className="size-6" />
                <span className="text-[13px] font-medium">{t('files.drop')}</span>
                <span className="text-xs text-fg-subtle">{t('files.dropHint')}</span>
              </button>
            </div>
          ) : (
            <div ref={listRef} className={cn('h-full overflow-auto', hovering && 'ring-2 ring-primary ring-inset')}>
              <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const file = files[row.index]
                  const result = results[file.path]
                  const source = sourceOf(file)
                  return (
                    <div
                      key={file.path}
                      role="button"
                      tabIndex={0}
                      onClick={() => setSelected(file.path)}
                      onKeyDown={(e) => e.key === 'Enter' && setSelected(file.path)}
                      className={cn(
                        'group absolute left-0 grid w-full cursor-default grid-cols-[minmax(0,1fr)_170px_64px_64px_80px_28px] items-center gap-2 border-b border-border px-3 outline-none',
                        file.path === selected ? 'bg-active' : 'hover:bg-hover',
                      )}
                      style={{ top: row.start, height: ROW_HEIGHT }}
                    >
                      <div className="min-w-0">
                        <p className="truncate text-[13px] text-fg">{file.name}</p>
                        <p className="truncate text-[11px] text-fg-subtle" title={file.path}>
                          {file.path}
                        </p>
                      </div>
                      {file.error ? (
                        <span className="col-span-3 truncate text-xs text-danger">{errorMessage(file.error)}</span>
                      ) : (
                        <>
                          <div className="flex min-w-0 items-center gap-1" onClick={(e) => e.stopPropagation()}>
                            <Select<string>
                              size="sm"
                              variant="ghost"
                              value={source}
                              onValueChange={(value) => setOverrides((prev) => ({ ...prev, [file.path]: value }))}
                              options={encodingOptions}
                              aria-label={t('files.encoding')}
                              className="min-w-0 flex-1"
                            />
                            {file.confidence === 'unsure' && !overrides[file.path] && (
                              <Tooltip content={t('files.unsure')}>
                                <AlertTriangle className="size-3.5 shrink-0 text-warning" />
                              </Tooltip>
                            )}
                          </div>
                          <span className="text-[11px] text-fg-muted">
                            {file.ascii && !overrides[file.path] ? 'ASCII' : file.bom ? t('files.bom') : ''}
                          </span>
                          <span className="text-[11px] text-fg-muted">{file.lineEndings && t(`lineEndings.${file.lineEndings}`)}</span>
                        </>
                      )}
                      <span className="text-right">
                        {result ? (
                          result.error ? (
                            <Tooltip content={errorMessage(result.error)}>
                              <Badge variant="danger">{t(`status.${result.status}`)}</Badge>
                            </Tooltip>
                          ) : (
                            <Badge variant={STATUS_BADGE[result.status]}>{t(`status.${result.status}`)}</Badge>
                          )
                        ) : (
                          <span className="text-[11px] text-fg-subtle tabular-nums">{formatBytes(file.size)}</span>
                        )}
                      </span>
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                        aria-label={t('files.remove')}
                        disabled={task.running}
                        onClick={(e) => {
                          e.stopPropagation()
                          remove(file.path)
                        }}
                      >
                        <X />
                      </Button>
                    </div>
                  )
                })}
              </div>
            </div>
          )}
        </Panel>
        <Panel
          icon={<Languages />}
          title={current ? t('preview.titleOf', { name: current.name }) : t('preview.title')}
          extra={
            preview.result && (
              <>
                {preview.result.malformed && (
                  <Tooltip content={t('preview.malformedHint')}>
                    <Badge variant="warning">{t('preview.malformed')}</Badge>
                  </Tooltip>
                )}
                {preview.result.unmappable && (
                  <Tooltip content={t('preview.unmappableHint', { ...preview.result.unmappable, encoding: t(`encodings.${to}`) })}>
                    <Badge variant="danger">{t('preview.unmappable', { char: preview.result.unmappable.char })}</Badge>
                  </Tooltip>
                )}
              </>
            )
          }
          className="min-h-0 flex-[2]"
          bodyClassName="overflow-auto"
        >
          {!current ? (
            <Empty icon={<Languages />} title={t('preview.empty')} description={t('preview.emptyHint')} />
          ) : preview.error ? (
            <p className="p-3 text-xs text-danger">{errorMessage(preview.error)}</p>
          ) : (
            <pre className={cn('p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap break-all text-fg', preview.pending && 'opacity-60')} data-selectable>
              {preview.result?.text}
              {preview.result?.truncated && <span className="text-fg-subtle">{'\n'}{t('preview.truncated')}</span>}
            </pre>
          )}
        </Panel>
      </div>

      <Panel icon={<RefreshCw />} title={t('convert.title')} bodyClassName="space-y-4 overflow-auto p-3">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('convert.to')}</p>
          <Select<string> value={to} onValueChange={setTo} options={encodingOptions} aria-label={t('convert.to')} className="w-full" />
          <label className={cn('flex items-center justify-between gap-2 pt-1 text-[13px]', bomAllowed ? 'text-fg' : 'text-fg-subtle')}>
            {t('convert.bom')}
            <Switch size="sm" checked={bomAllowed && bom} disabled={!bomAllowed} onCheckedChange={setBom} aria-label={t('convert.bom')} />
          </label>
          <p className="text-[11px] leading-snug text-fg-subtle">{t(bomAllowed ? 'convert.bomHint' : 'convert.bomUnavailable')}</p>
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('convert.newline')}</p>
          <SegmentedControl<NewLine>
            size="sm"
            value={newline}
            onValueChange={setNewline}
            aria-label={t('convert.newline')}
            options={(['keep', 'lf', 'crlf'] as NewLine[]).map((value) => ({ value, label: t(`newlines.${value}`) }))}
          />
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('convert.output')}</p>
          <SegmentedControl<Output>
            size="sm"
            value={output}
            onValueChange={setOutput}
            aria-label={t('convert.output')}
            options={(['inplace', 'folder'] as Output[]).map((value) => ({ value, label: t(`outputs.${value}`) }))}
          />
          {output === 'inplace' ? (
            <label className="flex items-center justify-between gap-2 pt-1 text-[13px] text-fg">
              {t('convert.backup')}
              <Switch size="sm" checked={backup} onCheckedChange={setBackup} aria-label={t('convert.backup')} />
            </label>
          ) : (
            <Button block className="justify-start" onClick={pickOutput}>
              <FolderOpen />
              <span className="truncate">{outputDir ?? t('convert.chooseFolder')}</span>
            </Button>
          )}
          <p className="text-[11px] leading-snug text-fg-subtle">{t(output === 'inplace' ? 'convert.inplaceHint' : 'convert.folderHint')}</p>
        </div>
        {convertible.length > 0 && (
          <div className="space-y-1">
            <p className="text-xs font-medium text-fg-muted">{t('convert.sources')}</p>
            <div className="flex flex-wrap gap-1">
              {Object.entries(counts)
                .sort((a, b) => b[1] - a[1])
                .map(([name, count]) => (
                  <Badge key={name}>
                    {name === 'ASCII' ? 'ASCII' : t(`encodings.${name}`)} · {count}
                  </Badge>
                ))}
            </div>
          </div>
        )}
        <Button
          block
          variant="primary"
          loading={task.running}
          disabled={convertible.length === 0 || (output === 'folder' && !outputDir)}
          onClick={start}
        >
          <RefreshCw />
          {t('convert.run', { count: convertible.length })}
        </Button>
        {output === 'folder' && outputDir && task.status === 'succeeded' && (
          <Button block size="sm" variant="ghost" onClick={() => host.revealPath(outputDir).catch(() => {})}>
            <FolderSearch />
            {t('convert.show')}
          </Button>
        )}
      </Panel>
    </div>
  )
}

export default TextEncoding
