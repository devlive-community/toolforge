import { useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, Checkbox, Empty, Input, Panel, Progress, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowUp, ChevronRight, Eye, File, FileArchive, Folder, FolderOpen, FolderOutput, KeyRound, Link2, Lock, ShieldAlert, Upload } from 'lucide-react'
import { ARCHIVE_EXTENSIONS, FORMAT_LABELS, type Child, type Conflict, type Extracted, type OpenInfo, type Preview } from './types'

const ROW_HEIGHT = 36
const dirOf = (path: string) => path.slice(0, Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')))

export function Browse({ dropped }: { dropped: string | null }) {
  const { t, call, errorMessage } = usePlugin()
  const opening = useTask<OpenInfo>()
  const extracting = useTask<Extracted>()
  const [archive, setArchive] = useState<OpenInfo | null>(null)
  const [pending, setPending] = useState<string | null>(null)
  const [password, setPassword] = useState('')
  const [dir, setDir] = useState('')
  const [children, setChildren] = useState<Child[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [preview, setPreview] = useState<{ entry: Child; data: Preview | null } | null>(null)
  const [dest, setDest] = useState<string | null>(null)
  const [createFolder, setCreateFolder] = useState(true)
  const [conflict, setConflict] = useState<Conflict>('rename')
  const listRef = useRef<HTMLDivElement>(null)

  const open = (path: string, withPassword?: string) => {
    setPending(path)
    opening.start('open', { path, password: withPassword || null })
  }
  useEffect(() => {
    if (dropped) open(dropped)
  }, [dropped]) // eslint-disable-line react-hooks/exhaustive-deps

  const [seenOpen, setSeenOpen] = useState<OpenInfo | null>(null)
  if (opening.status === 'succeeded' && opening.result && opening.result !== seenOpen) {
    setSeenOpen(opening.result)
    setArchive(opening.result)
    setDir('')
    setSelected(new Set())
    setPreview(null)
    setDest(null)
  }
  const needsPassword = opening.status === 'failed' && (opening.error?.code === 'archive.password_required' || opening.error?.code === 'archive.wrong_password')
  const extractNeedsPassword = extracting.status === 'failed' && (extracting.error?.code === 'archive.password_required' || extracting.error?.code === 'archive.wrong_password')

  useEffect(() => {
    if (opening.status === 'failed' && opening.error && !needsPassword) toast.error(errorMessage(opening.error))
    if (extracting.status === 'failed' && extracting.error && !extractNeedsPassword) toast.error(errorMessage(extracting.error))
    if (extracting.status === 'succeeded' && extracting.result) toast.success(t('extract.done', { count: extracting.result.files, size: formatBytes(extracting.result.bytes) }))
  }, [opening.status, extracting.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const archiveId = archive?.id
  useEffect(() => {
    if (archiveId === undefined) return
    let alive = true
    call<{ children: Child[] }>('list', { id: archiveId, dir })
      .then((result) => alive && setChildren(result.children))
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
  }, [archiveId, dir, call, errorMessage])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('open.filter'), extensions: ARCHIVE_EXTENSIONS }])
      if (path) open(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickDest = async () => {
    try {
      const path = await host.dialog.openDirectory()
      if (path) setDest(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const showPreview = (entry: Child) => {
    if (!archive) return
    setPreview({ entry, data: null })
    call<Preview>('preview', { id: archive.id, entry: entry.path, password: password || null })
      .then((data) => setPreview((prev) => (prev?.entry === entry ? { entry, data } : prev)))
      .catch((error) => {
        setPreview(null)
        toast.error(errorMessage(error))
      })
  }

  const extract = (entries: string[]) => {
    if (!archive) return
    extracting.start('extract', { id: archive.id, dest: dest ?? dirOf(archive.path), entries, conflict, createFolder, password: password || null })
  }

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: children.length, getScrollElement: () => listRef.current, estimateSize: () => ROW_HEIGHT, overscan: 12 })

  if (!archive) {
    return (
      <div className="flex h-full min-h-0 flex-col gap-3">
        {needsPassword && pending ? (
          <Panel icon={<KeyRound />} title={t('password.title')} className="shrink-0" bodyClassName="flex flex-wrap items-center gap-2 p-3">
            <span className="text-[13px] text-fg-muted">{t(opening.error?.code === 'archive.wrong_password' ? 'password.wrong' : 'password.required')}</span>
            <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && open(pending, password)} autoFocus wrapperClassName="w-60" aria-label={t('password.title')} />
            <Button variant="primary" onClick={() => open(pending, password)} disabled={!password}>
              {t('password.unlock')}
            </Button>
          </Panel>
        ) : null}
        <button
          type="button"
          onClick={pick}
          disabled={opening.running}
          className="flex min-h-56 flex-1 flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed border-border text-center text-fg-muted outline-none transition-colors hover:border-border-strong hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
        >
          <Upload className="size-6" />
          <span className="text-[13px] font-medium">{opening.running ? t('open.reading') : t('open.drop')}</span>
          {opening.running ? (
            <span className="w-64">
              <Progress value={opening.progress && opening.progress.total > 0 ? (opening.progress.done / opening.progress.total) * 100 : null} />
            </span>
          ) : (
            <span className="text-xs text-fg-subtle">{t('open.hint')}</span>
          )}
        </button>
      </div>
    )
  }

  const crumbs = dir ? dir.split('/') : []
  const allSelected = children.length > 0 && children.every((c) => selected.has(c.path))
  const toggle = (path: string) => {
    const next = new Set(selected)
    if (next.has(path)) next.delete(path)
    else next.add(path)
    setSelected(next)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3 shadow-card">
        <FileArchive className="size-5 text-primary" />
        <span className="min-w-0 truncate text-[13px] font-medium text-fg" title={archive.path}>
          {archive.name}
        </span>
        <Badge>{FORMAT_LABELS[archive.format] ?? archive.format}</Badge>
        <span className="text-xs text-fg-muted tabular-nums">{t('open.summary', { files: archive.files.toLocaleString(), bytes: formatBytes(archive.bytes), size: formatBytes(archive.size) })}</span>
        {archive.encrypted && (
          <Badge variant="info">
            <Lock className="size-3" />
            {t('open.encrypted')}
          </Badge>
        )}
        {archive.unsafePaths > 0 && (
          <Tooltip content={t('open.unsafeHint')}>
            <Badge variant="danger">
              <ShieldAlert className="size-3" />
              {t('open.unsafe', { count: archive.unsafePaths })}
            </Badge>
          </Tooltip>
        )}
        {archive.links > 0 && (
          <Tooltip content={t('open.linksHint')}>
            <Badge variant="warning">
              <Link2 className="size-3" />
              {t('open.links', { count: archive.links })}
            </Badge>
          </Tooltip>
        )}
        <Button size="sm" className="ml-auto" onClick={pick} disabled={opening.running}>
          <FolderOpen />
          {t('open.another')}
        </Button>
      </section>

      <div className={cn('grid min-h-0 flex-1 gap-3', preview ? 'grid-cols-[minmax(0,1fr)_360px]' : 'grid-cols-1')}>
        <Panel
          title={
            <span className="flex min-w-0 items-center gap-0.5">
              <button type="button" className="rounded-sm px-1 text-fg-muted outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring" onClick={() => setDir('')}>
                {archive.name}
              </button>
              {crumbs.map((part, index) => (
                <span key={index} className="flex min-w-0 items-center gap-0.5">
                  <ChevronRight className="size-3.5 shrink-0 text-fg-subtle" />
                  <button type="button" className={cn('truncate rounded-sm px-1 outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring', index === crumbs.length - 1 ? 'text-fg' : 'text-fg-muted')} onClick={() => setDir(crumbs.slice(0, index + 1).join('/'))}>
                    {part}
                  </button>
                </span>
              ))}
            </span>
          }
          actions={
            <Button size="icon-sm" variant="ghost" aria-label={t('browse.up')} disabled={!dir} onClick={() => setDir(crumbs.slice(0, -1).join('/'))}>
              <ArrowUp />
            </Button>
          }
          footer={
            <div className="flex w-full flex-wrap items-center gap-3">
              <Button size="sm" className="min-w-0 max-w-64 justify-start" onClick={pickDest}>
                <FolderOutput />
                <span className="truncate">{dest ?? t('extract.nextTo')}</span>
              </Button>
              <label className="flex items-center gap-1.5 text-xs text-fg">
                <Switch size="sm" checked={createFolder} onCheckedChange={setCreateFolder} aria-label={t('extract.createFolder')} />
                {t('extract.createFolder')}
              </label>
              <Select<Conflict>
                size="sm"
                value={conflict}
                onValueChange={setConflict}
                aria-label={t('extract.conflict')}
                options={(['rename', 'overwrite', 'skip'] as Conflict[]).map((value) => ({ value, label: t(`conflicts.${value}`) }))}
              />
              <span className="ml-auto flex items-center gap-2">
                {extracting.running && (
                  <span className="w-32">
                    <Progress value={extracting.progress && extracting.progress.total > 0 ? (extracting.progress.done / extracting.progress.total) * 100 : null} />
                  </span>
                )}
                {extracting.status === 'succeeded' && extracting.result && (
                  <Button size="sm" variant="ghost" onClick={() => host.revealPath(extracting.result!.dest).catch((error) => toast.error(errorMessage(error)))}>
                    {t('extract.show')}
                  </Button>
                )}
                <Button size="sm" disabled={extracting.running || selected.size === 0} onClick={() => extract([...selected])}>
                  {t('extract.selected', { count: selected.size })}
                </Button>
                <Button size="sm" variant="primary" loading={extracting.running} onClick={() => extract([])}>
                  {t('extract.all')}
                </Button>
              </span>
              {extractNeedsPassword && (
                <span className="flex w-full flex-wrap items-center gap-2">
                  <KeyRound className="size-4 text-danger" />
                  <span className="text-xs text-danger">{t(extracting.error?.code === 'archive.wrong_password' ? 'password.wrong' : 'password.required')}</span>
                  <Input size="sm" type="password" value={password} onChange={(e) => setPassword(e.target.value)} wrapperClassName="w-48" aria-label={t('password.title')} />
                </span>
              )}
            </div>
          }
          bodyClassName="p-0"
        >
          <div className="grid grid-cols-[28px_minmax(0,1fr)_96px_96px_132px] items-center gap-2 border-b border-border bg-surface-2 px-3 py-1.5 text-[11px] font-medium text-fg-muted">
            <Checkbox checked={allSelected} indeterminate={!allSelected && children.some((c) => selected.has(c.path))} onCheckedChange={(on) => setSelected(on ? new Set([...selected, ...children.map((c) => c.path)]) : new Set([...selected].filter((p) => !children.some((c) => c.path === p))))} aria-label={t('browse.selectAll')} />
            <span>{t('browse.name')}</span>
            <span className="text-right">{t('browse.size')}</span>
            <span className="text-right">{t('browse.packed')}</span>
            <span className="pl-3">{t('browse.modified')}</span>
          </div>
          {children.length === 0 ? (
            <Empty title={t('browse.empty')} />
          ) : (
            <div ref={listRef} className="h-[calc(100%-30px)] overflow-auto">
              <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const child = children[row.index]
                  const Icon = child.kind === 'dir' ? Folder : child.kind === 'link' ? Link2 : File
                  return (
                    <div
                      key={row.key}
                      className={cn('absolute left-0 grid w-full grid-cols-[28px_minmax(0,1fr)_96px_96px_132px] items-center gap-2 border-b border-border px-3 hover:bg-hover', preview?.entry.path === child.path && 'bg-primary-soft')}
                      style={{ top: row.start, height: ROW_HEIGHT }}
                      onDoubleClick={() => (child.kind === 'dir' ? setDir(child.path) : showPreview(child))}
                    >
                      <Checkbox checked={selected.has(child.path)} onCheckedChange={() => toggle(child.path)} aria-label={t('browse.select')} />
                      <button
                        type="button"
                        className="flex min-w-0 items-center gap-2 text-left text-[13px] text-fg outline-none"
                        onClick={() => (child.kind === 'dir' ? setDir(child.path) : child.kind === 'file' && child.safe ? showPreview(child) : undefined)}
                      >
                        <Icon className={cn('size-4 shrink-0', child.kind === 'dir' ? 'text-primary' : 'text-fg-subtle')} />
                        <span className={cn('truncate', !child.safe && 'text-danger line-through')}>{child.name}</span>
                        {child.encrypted && <Lock className="size-3 shrink-0 text-info" />}
                        {child.kind === 'dir' && <span className="shrink-0 text-[11px] text-fg-subtle">{t('browse.files', { count: child.files })}</span>}
                      </button>
                      <span className="text-right text-xs text-fg-muted tabular-nums">{formatBytes(child.size)}</span>
                      <span className="text-right text-xs text-fg-subtle tabular-nums">{child.packed !== null ? formatBytes(child.packed) : '—'}</span>
                      <span className="truncate pl-3 text-xs text-fg-subtle tabular-nums">{child.modified ?? ''}</span>
                    </div>
                  )
                })}
              </div>
            </div>
          )}
        </Panel>

        {preview && (
          <Panel
            icon={<Eye />}
            title={preview.entry.name}
            extra={<Badge>{formatBytes(preview.entry.size)}</Badge>}
            actions={
              <Button size="sm" variant="ghost" onClick={() => setPreview(null)}>
                {t('preview.close')}
              </Button>
            }
            bodyClassName="overflow-auto p-3"
          >
            {!preview.data ? (
              <p className="text-xs text-fg-muted">{t('preview.loading')}</p>
            ) : preview.data.kind === 'text' ? (
              <>
                <pre className="font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
                  {preview.data.text}
                </pre>
                {preview.data.truncated && <p className="mt-2 text-xs text-warning">{t('preview.truncated')}</p>}
              </>
            ) : preview.data.kind === 'image' && preview.data.dataUri ? (
              <img src={preview.data.dataUri} alt={preview.entry.name} className="max-w-full rounded-control border border-border" />
            ) : (
              <p className="text-xs text-fg-muted">{t(`preview.${preview.data.kind}`)}</p>
            )}
          </Panel>
        )}
      </div>
    </div>
  )
}
