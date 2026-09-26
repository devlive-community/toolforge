import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, Checkbox, Empty, Modal, Panel, Progress, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { Copy as CopyIcon, FolderPlus, FolderSearch, Link2, Play, Square, Trash2, Wand2, X } from 'lucide-react'
import { DEFAULT_SETTINGS, KIND_EXTENSIONS, MIN_SIZES, autoSelect, type Group, type Kind, type Outcome, type Report, type Settings, type Strategy } from './types'

const HEADER_HEIGHT = 40
const FILE_HEIGHT = 44
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const dirName = (path: string) => path.slice(0, path.length - baseName(path).length)

type Row = { type: 'group'; group: Group; index: number } | { type: 'file'; group: Group; path: string }

function DuplicateFinder() {
  const { t, errorMessage } = usePlugin()
  const [settings, setSettings] = usePluginState<Settings>('settings', DEFAULT_SETTINGS)
  const [strategy, setStrategy] = usePluginState<Strategy>('strategy', 'newest')
  const scanning = useTask<Report>()
  const removing = useTask<Outcome>()
  const [groups, setGroups] = useState<Group[] | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [confirming, setConfirming] = useState(false)
  const listRef = useRef<HTMLDivElement>(null)
  const update = (patch: Partial<Settings>) => setSettings({ ...settings, ...patch })

  const [seenScan, setSeenScan] = useState<Report | null>(null)
  if (scanning.status === 'succeeded' && scanning.result && scanning.result !== seenScan) {
    setSeenScan(scanning.result)
    setGroups(scanning.result.groups)
    setSelected(new Set(scanning.result.groups.flatMap((g) => autoSelect(g, strategy))))
  }
  const [seenRemove, setSeenRemove] = useState<Outcome | null>(null)
  if (removing.status === 'succeeded' && removing.result && removing.result !== seenRemove) {
    const removed = new Set(removing.result.removed)
    setSeenRemove(removing.result)
    setGroups((prev) =>
      (prev ?? [])
        .map((g) => ({ ...g, files: g.files.filter((f) => !removed.has(f.path)) }))
        .filter((g) => g.files.length > 1)
        .map((g) => ({ ...g, wasted: g.size * (g.files.length - 1) })),
    )
    setSelected((prev) => new Set([...prev].filter((p) => !removed.has(p))))
  }

  useEffect(() => {
    if (removing.status === 'succeeded' && removing.result) {
      const { removed, skipped, freed } = removing.result
      toast.success(t('remove.done', { count: removed.length, size: formatBytes(freed) }))
      if (skipped.length > 0) toast.error(t('remove.skipped', { count: skipped.length }))
    }
    for (const task of [scanning, removing]) {
      if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
    }
  }, [scanning.status, removing.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const addFolder = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir && !settings.roots.includes(dir)) update({ roots: [...settings.roots, dir] })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const start = () =>
    scanning.start('scan', {
      roots: settings.roots,
      minSize: settings.minSize,
      includeHidden: settings.includeHidden,
      extensions: KIND_EXTENSIONS[settings.kind],
    })

  const rows = useMemo<Row[]>(
    () => (groups ?? []).flatMap((group, index) => [{ type: 'group' as const, group, index }, ...group.files.map((f) => ({ type: 'file' as const, group, path: f.path }))]),
    [groups],
  )
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => listRef.current,
    estimateSize: (index) => (rows[index].type === 'group' ? HEADER_HEIGHT : FILE_HEIGHT),
    overscan: 16,
  })

  const toggle = (group: Group, path: string) => {
    const next = new Set(selected)
    if (next.has(path)) {
      next.delete(path)
    } else {
      // 每组至少保留一个文件
      if (group.files.filter((f) => f.path !== path).every((f) => next.has(f.path))) {
        toast.error(t('select.keepOne'))
        return
      }
      next.add(path)
    }
    setSelected(next)
  }
  const applyStrategy = (value: Strategy) => {
    setStrategy(value)
    setSelected(new Set((groups ?? []).flatMap((g) => autoSelect(g, value))))
  }

  const selection = (groups ?? []).filter((g) => g.files.some((f) => selected.has(f.path)))
  const freed = selection.reduce((sum, g) => sum + g.size * g.files.filter((f) => selected.has(f.path)).length, 0)
  const remove = () => {
    setConfirming(false)
    removing.start('remove', {
      groups: selection.map((g) => ({
        hash: g.hash,
        keep: g.files.filter((f) => !selected.has(f.path)).map((f) => f.path),
        remove: g.files.filter((f) => selected.has(f.path)).map((f) => f.path),
      })),
    })
  }
  const reveal = (path: string) => host.revealPath(path).catch((error) => toast.error(errorMessage(error)))

  const busy = scanning.running || removing.running
  const task = scanning.running ? scanning : removing
  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const report = scanning.result

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-col gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-2">
          {settings.roots.map((root) => (
            <span key={root} className="flex max-w-80 items-center gap-1 rounded-control border border-border bg-surface-2 py-0.5 pr-0.5 pl-2 text-xs text-fg">
              <span className="truncate" title={root}>
                {root}
              </span>
              <Button size="icon-sm" variant="ghost" aria-label={t('folders.remove')} disabled={busy} onClick={() => update({ roots: settings.roots.filter((r) => r !== root) })}>
                <X />
              </Button>
            </span>
          ))}
          <Button size="sm" onClick={addFolder} disabled={busy}>
            <FolderPlus />
            {t('folders.add')}
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Select<Kind>
            size="sm"
            value={settings.kind}
            onValueChange={(kind) => update({ kind })}
            aria-label={t('options.kind')}
            options={(Object.keys(KIND_EXTENSIONS) as Kind[]).map((value) => ({ value, label: t(`kinds.${value}`) }))}
          />
          <Select<number>
            size="sm"
            value={settings.minSize}
            onValueChange={(minSize) => update({ minSize })}
            aria-label={t('options.minSize')}
            options={MIN_SIZES.map((value) => ({ value, label: value === 1 ? t('options.anySize') : t('options.atLeast', { size: formatBytes(value) }) }))}
          />
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={settings.includeHidden} onCheckedChange={(includeHidden) => update({ includeHidden })} aria-label={t('options.hidden')} />
            {t('options.hidden')}
          </label>
          <span className="ml-auto flex items-center gap-3">
            {busy && (
              <span className="flex w-56 flex-col gap-1">
                <Progress value={progress} />
                <span className="text-[11px] text-fg-muted">{task.stage ? t(`stages.${task.stage}`) : ''}</span>
              </span>
            )}
            {scanning.running ? (
              <Button onClick={scanning.cancel}>
                <Square />
                {t('scan.cancel')}
              </Button>
            ) : (
              <Button variant="primary" onClick={start} disabled={busy || settings.roots.length === 0}>
                <Play />
                {t('scan.start')}
              </Button>
            )}
          </span>
        </div>
      </section>

      <Panel
        icon={<CopyIcon />}
        title={t('results.title')}
        extra={
          groups &&
          report && (
            <Badge variant={groups.length > 0 ? 'warning' : 'success'}>
              {groups.length > 0 ? t('results.summary', { groups: groups.length, wasted: formatBytes(groups.reduce((s, g) => s + g.wasted, 0)) }) : t('results.clean')}
            </Badge>
          )
        }
        actions={
          groups &&
          groups.length > 0 && (
            <>
              <Wand2 className="size-4 text-fg-subtle" />
              <Select<Strategy>
                size="sm"
                value={strategy}
                onValueChange={applyStrategy}
                aria-label={t('select.strategy')}
                options={(['newest', 'oldest', 'shortest', 'firstFolder'] as Strategy[]).map((value) => ({ value, label: t(`strategies.${value}`) }))}
              />
              <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())} disabled={selected.size === 0}>
                {t('select.clear')}
              </Button>
            </>
          )
        }
        footer={
          groups &&
          groups.length > 0 && (
            <>
              <span className="tabular-nums">
                {report && t('results.scanned', { files: report.scanned.toLocaleString(), size: formatBytes(report.scannedBytes), ms: report.elapsedMs })}
              </span>
              <span className="ml-auto flex items-center gap-3">
                <span className="tabular-nums">{t('select.summary', { count: selected.size, size: formatBytes(freed) })}</span>
                <Button size="sm" variant="danger" disabled={busy || selected.size === 0} onClick={() => setConfirming(true)}>
                  <Trash2 />
                  {t('remove.action')}
                </Button>
              </span>
            </>
          )
        }
        className="min-h-0 flex-1"
        bodyClassName="p-0"
      >
        {!groups ? (
          <Empty icon={<CopyIcon />} title={scanning.running ? t('scan.running') : t('results.empty')} description={scanning.running ? undefined : t('results.emptyHint')} />
        ) : groups.length === 0 ? (
          <Empty icon={<CopyIcon />} title={t('results.clean')} description={t('results.cleanHint')} />
        ) : (
          <div ref={listRef} className="h-full overflow-auto">
            <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
              {virtualizer.getVirtualItems().map((item) => {
                const row = rows[item.index]
                const style = { top: item.start, height: item.size }
                if (row.type === 'group') {
                  return (
                    <div key={item.key} className="absolute left-0 flex w-full items-center gap-2 border-b border-border bg-surface-2 px-3 text-xs text-fg-muted" style={style}>
                      <span className="font-medium text-fg tabular-nums">{t('results.copies', { count: row.group.files.length, size: formatBytes(row.group.size) })}</span>
                      <Badge variant="warning">{t('results.wasted', { size: formatBytes(row.group.wasted) })}</Badge>
                      <span className="ml-auto font-mono text-[11px] text-fg-subtle" title={row.group.hash}>
                        {row.group.hash.slice(0, 12)}
                      </span>
                    </div>
                  )
                }
                const file = row.group.files.find((f) => f.path === row.path)!
                const checked = selected.has(file.path)
                return (
                  <div key={item.key} className={cn('absolute left-0 flex w-full items-center gap-3 border-b border-border px-3 hover:bg-hover', checked && 'bg-danger-soft/40')} style={style}>
                    <Checkbox checked={checked} onCheckedChange={() => toggle(row.group, file.path)} aria-label={t('select.toggle')} disabled={busy} />
                    <div className="min-w-0 flex-1">
                      <p className={cn('truncate text-[13px]', checked ? 'text-fg-muted line-through' : 'text-fg')}>{baseName(file.path)}</p>
                      <p className="truncate text-[11px] text-fg-subtle" title={file.path} data-selectable>
                        {dirName(file.path)}
                      </p>
                    </div>
                    {file.links.length > 0 && (
                      <Tooltip content={t('results.linksHint', { paths: file.links.join('\n') })}>
                        <Badge>
                          <Link2 className="size-3" />
                          {file.links.length}
                        </Badge>
                      </Tooltip>
                    )}
                    <span className="shrink-0 text-[11px] text-fg-subtle tabular-nums">{new Date(file.modified).toLocaleString()}</span>
                    <Tooltip content={t('results.reveal')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('results.reveal')} onClick={() => reveal(file.path)}>
                        <FolderSearch />
                      </Button>
                    </Tooltip>
                  </div>
                )
              })}
            </div>
          </div>
        )}
      </Panel>

      <Modal
        open={confirming}
        onOpenChange={setConfirming}
        title={t('remove.confirmTitle', { count: selected.size })}
        description={t('remove.confirmText', { size: formatBytes(freed) })}
        closeLabel={t('common:close')}
        size="sm"
        footer={
          <>
            <Button onClick={() => setConfirming(false)}>{t('remove.cancel')}</Button>
            <Button variant="danger" onClick={remove}>
              <Trash2 />
              {t('remove.confirm')}
            </Button>
          </>
        }
      />
    </div>
  )
}

export default DuplicateFinder
