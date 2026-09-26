import { useEffect, useState } from 'react'
import { Badge, Button, Checkbox, Empty, Modal, Panel, Progress, Switch, Tabs, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowUp, ChevronRight, File, Folder, FolderOpen, FolderSearch, HardDrive, Play, Square, Trash2 } from 'lucide-react'
import { Treemap } from './Treemap'
import { CATEGORY_CLASS, type LargeFile, type Listing, type ScanResult, type Settings, type TypeStat } from './types'

type Tab = 'contents' | 'largest' | 'types'

function Bar({ value, className }: { value: number; className?: string }) {
  return (
    <span className="block h-1 w-full overflow-hidden rounded-full bg-active">
      <span className={cn('block h-full rounded-full bg-primary', className)} style={{ width: `${Math.max(0.5, value * 100)}%` }} />
    </span>
  )
}

function DiskUsage() {
  const { t, call, errorMessage } = usePlugin()
  const [settings, setSettings] = usePluginState<Settings>('settings', { root: '', includeHidden: true })
  const scanning = useTask<ScanResult>()
  const trashing = useTask<{ removed: number[]; freed: number; failed: number }>()
  const [scan, setScan] = useState<ScanResult | null>(null)
  const [node, setNode] = useState(0)
  const [version, setVersion] = useState(0)
  const [tab, setTab] = useState<Tab>('contents')
  const [listing, setListing] = useState<Listing | null>(null)
  const [largest, setLargest] = useState<LargeFile[]>([])
  const [types, setTypes] = useState<TypeStat[]>([])
  const [selected, setSelected] = useState<Map<number, { name: string; size: number }>>(new Map())
  const [confirming, setConfirming] = useState(false)

  const [seenScan, setSeenScan] = useState<ScanResult | null>(null)
  if (scanning.status === 'succeeded' && scanning.result && scanning.result !== seenScan) {
    setSeenScan(scanning.result)
    setScan(scanning.result)
    setNode(0)
    setSelected(new Map())
  }
  const [seenTrash, setSeenTrash] = useState<unknown>(null)
  if (trashing.status === 'succeeded' && trashing.result && trashing.result !== seenTrash) {
    setSeenTrash(trashing.result)
    setSelected(new Map())
    setVersion((v) => v + 1)
  }

  useEffect(() => {
    if (trashing.status === 'succeeded' && trashing.result) {
      toast.success(t('trash.done', { count: trashing.result.removed.length, size: formatBytes(trashing.result.freed) }))
      if (trashing.result.failed > 0) toast.error(t('trash.failed', { count: trashing.result.failed }))
    }
    for (const task of [scanning, trashing]) if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [scanning.status, trashing.status]) // eslint-disable-line react-hooks/exhaustive-deps

  // 当前文件夹变化或移除文件后重新读取
  const scanId = scan?.id
  useEffect(() => {
    if (scanId === undefined) return
    let alive = true
    Promise.all([
      call<Listing>('list', { id: scanId, node }),
      call<LargeFile[]>('largest', { id: scanId, node, limit: 100 }),
      call<TypeStat[]>('types', { id: scanId, node }),
    ])
      .then(([l, big, stats]) => {
        if (!alive) return
        setListing(l)
        setLargest(big)
        setTypes(stats)
      })
      .catch((error) => {
        // 当前节点被移除时回到上一级
        if (alive) setNode(0)
        toast.error(errorMessage(error))
      })
    return () => {
      alive = false
    }
  }, [scanId, node, version, call, errorMessage])

  const pick = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setSettings({ ...settings, root: dir })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const start = () => scanning.start('scan', { root: settings.root, includeHidden: settings.includeHidden })

  const toggle = (id: number, name: string, size: number) => {
    const next = new Map(selected)
    if (next.has(id)) next.delete(id)
    else next.set(id, { name, size })
    setSelected(next)
  }
  const selectedSize = [...selected.values()].reduce((sum, item) => sum + item.size, 0)
  const reveal = (path: string) => host.revealPath(path).catch((error) => toast.error(errorMessage(error)))

  const busy = scanning.running || trashing.running
  const total = listing?.node.size || 1

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
        <Button className="min-w-0 max-w-[50%] justify-start" onClick={pick} disabled={busy}>
          <FolderOpen />
          <span className="truncate">{settings.root || t('scan.choose')}</span>
        </Button>
        <label className="flex items-center gap-2 text-[13px] text-fg">
          <Switch size="sm" checked={settings.includeHidden} onCheckedChange={(includeHidden) => setSettings({ ...settings, includeHidden })} aria-label={t('scan.hidden')} />
          {t('scan.hidden')}
        </label>
        <span className="ml-auto flex items-center gap-3">
          {scanning.running && (
            <span className="flex w-48 flex-col gap-1">
              <Progress value={null} />
              <span className="text-[11px] text-fg-muted tabular-nums">{t('scan.progress', { count: (scanning.progress?.done ?? 0).toLocaleString() })}</span>
            </span>
          )}
          {scanning.running ? (
            <Button onClick={scanning.cancel}>
              <Square />
              {t('scan.cancel')}
            </Button>
          ) : (
            <Button variant="primary" onClick={start} disabled={busy || !settings.root}>
              <Play />
              {t('scan.start')}
            </Button>
          )}
        </span>
      </section>

      {!scan || !listing ? (
        <Panel className="min-h-0 flex-1">
          <Empty icon={<HardDrive />} title={scanning.running ? t('scan.running') : t('empty.title')} description={scanning.running ? undefined : t('empty.hint')} />
        </Panel>
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_380px] gap-3">
          <Panel
            icon={<HardDrive />}
            title={
              <span className="flex min-w-0 items-center gap-0.5">
                {listing.crumbs.map((crumb, index) => (
                  <span key={crumb.id} className="flex min-w-0 items-center gap-0.5">
                    {index > 0 && <ChevronRight className="size-3.5 shrink-0 text-fg-subtle" />}
                    <button
                      type="button"
                      onClick={() => setNode(crumb.id)}
                      className={cn('truncate rounded-sm px-1 outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring', index === listing.crumbs.length - 1 ? 'text-fg' : 'text-fg-muted')}
                      title={crumb.name}
                    >
                      {index === 0 ? crumb.name.split(/[\\/]/).filter(Boolean).pop() || crumb.name : crumb.name}
                    </button>
                  </span>
                ))}
              </span>
            }
            extra={<Badge>{formatBytes(listing.node.size)}</Badge>}
            actions={
              <Tooltip content={t('nav.up')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('nav.up')} disabled={listing.crumbs.length < 2} onClick={() => setNode(listing.crumbs[listing.crumbs.length - 2].id)}>
                  <ArrowUp />
                </Button>
              </Tooltip>
            }
            footer={
              <>
                <span className="tabular-nums">{t('summary.files', { files: listing.node.files.toLocaleString(), ms: scan.elapsedMs })}</span>
                {scan.denied > 0 && <span className="text-warning">{t('summary.denied', { count: scan.denied })}</span>}
                <span className="ml-auto flex flex-wrap items-center gap-2">
                  {types.map((stat) => (
                    <span key={stat.category} className="flex items-center gap-1 text-[11px]">
                      <span className={cn('size-2.5 rounded-[3px]', CATEGORY_CLASS[stat.category])} />
                      {t(`categories.${stat.category}`)}
                    </span>
                  ))}
                </span>
              </>
            }
            bodyClassName="p-2"
          >
            <Treemap scan={scan.id} node={node} version={version} selected={new Set(selected.keys())} onOpen={setNode} />
          </Panel>

          <Panel
            title={
              <Tabs<Tab>
                value={tab}
                onValueChange={setTab}
                aria-label={t('tabs.label')}
                items={[
                  { value: 'contents', label: t('tabs.contents') },
                  { value: 'largest', label: t('tabs.largest') },
                  { value: 'types', label: t('tabs.types') },
                ]}
              />
            }
            footer={
              <>
                <span className="tabular-nums">{t('trash.summary', { count: selected.size, size: formatBytes(selectedSize) })}</span>
                <Button size="sm" variant="danger" className="ml-auto" disabled={busy || selected.size === 0} onClick={() => setConfirming(true)}>
                  <Trash2 />
                  {t('trash.action')}
                </Button>
              </>
            }
            bodyClassName="overflow-auto p-1.5"
          >
            {tab === 'contents' && (
              <ul>
                {listing.children.map((child) => (
                  <li key={child.id} className="group flex items-center gap-2 rounded-control px-2 py-1.5 hover:bg-hover">
                    <Checkbox checked={selected.has(child.id)} onCheckedChange={() => toggle(child.id, child.name, child.size)} aria-label={t('trash.select')} disabled={busy} />
                    <button type="button" className="flex min-w-0 flex-1 flex-col gap-1 text-left outline-none" disabled={!child.dir} onClick={() => child.dir && setNode(child.id)}>
                      <span className="flex items-center gap-1.5 text-[13px] text-fg">
                        {child.dir ? <Folder className="size-3.5 shrink-0 text-primary" /> : <File className="size-3.5 shrink-0 text-fg-subtle" />}
                        <span className="truncate">{child.name}</span>
                        <span className="ml-auto shrink-0 text-xs text-fg-muted tabular-nums">{formatBytes(child.size)}</span>
                      </span>
                      <Bar value={child.size / total} className={child.dir ? undefined : CATEGORY_CLASS[child.category]} />
                    </button>
                    <Tooltip content={t('reveal')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('reveal')} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={() => reveal(child.path)}>
                        <FolderSearch />
                      </Button>
                    </Tooltip>
                  </li>
                ))}
                {listing.others && <li className="px-2 py-1.5 text-xs text-fg-subtle">{t('list.others', { count: listing.others[0], size: formatBytes(listing.others[1]) })}</li>}
                {listing.children.length === 0 && <Empty title={t('list.empty')} />}
              </ul>
            )}
            {tab === 'largest' && (
              <ul>
                {largest.map((file) => (
                  <li key={file.id} className="group flex items-center gap-2 rounded-control px-2 py-1.5 hover:bg-hover">
                    <Checkbox checked={selected.has(file.id)} onCheckedChange={() => toggle(file.id, file.name, file.size)} aria-label={t('trash.select')} disabled={busy} />
                    <span className={cn('size-2.5 shrink-0 rounded-[3px]', CATEGORY_CLASS[file.category])} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] text-fg">{file.name}</span>
                      <span className="block truncate text-[11px] text-fg-subtle" title={file.path}>
                        {file.path}
                      </span>
                    </span>
                    <span className="shrink-0 text-xs text-fg-muted tabular-nums">{formatBytes(file.size)}</span>
                    <Tooltip content={t('reveal')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('reveal')} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={() => reveal(file.path)}>
                        <FolderSearch />
                      </Button>
                    </Tooltip>
                  </li>
                ))}
              </ul>
            )}
            {tab === 'types' && (
              <ul className="space-y-3 p-2">
                {types.map((stat) => (
                  <li key={stat.category} className="space-y-1.5">
                    <span className="flex items-center gap-2 text-[13px] text-fg">
                      <span className={cn('size-3 rounded-[3px]', CATEGORY_CLASS[stat.category])} />
                      {t(`categories.${stat.category}`)}
                      <span className="text-xs text-fg-subtle tabular-nums">{t('types.files', { count: stat.files })}</span>
                      <span className="ml-auto text-xs text-fg-muted tabular-nums">{formatBytes(stat.size)}</span>
                    </span>
                    <Bar value={stat.size / total} className={CATEGORY_CLASS[stat.category]} />
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        </div>
      )}

      <Modal
        open={confirming}
        onOpenChange={setConfirming}
        title={t('trash.confirmTitle', { count: selected.size })}
        description={t('trash.confirmText', { size: formatBytes(selectedSize), names: [...selected.values()].slice(0, 5).map((s) => s.name).join('、') })}
        closeLabel={t('common:close')}
        size="sm"
        footer={
          <>
            <Button onClick={() => setConfirming(false)}>{t('trash.cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirming(false)
                if (scan) trashing.start('trash', { id: scan.id, nodes: [...selected.keys()] })
              }}
            >
              <Trash2 />
              {t('trash.confirm')}
            </Button>
          </>
        }
      />
    </div>
  )
}

export default DiskUsage
