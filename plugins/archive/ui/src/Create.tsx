import { useEffect, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Progress, SegmentedControl, Select, Tooltip, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { FilePlus2, FolderPlus, FolderSearch, PackagePlus, Save, X } from 'lucide-react'
import type { Created, Output } from './types'

const LEVELS = [0, 3, 6, 9]
const EXTENSION: Record<Output, string> = { zip: 'zip', sevenZ: '7z', tarGz: 'tar.gz', tarXz: 'tar.xz' }

export function Create({ dropped }: { dropped: string[] }) {
  const { t, call, errorMessage } = usePlugin()
  const [format, setFormat] = usePluginState<Output>('format', 'zip')
  const [level, setLevel] = usePluginState<number>('level', 6)
  const [sources, setSources] = useState<string[]>([])
  const [password, setPassword] = useState('')
  const [output, setOutput] = useState<{ path: string; custom: boolean } | null>(null)
  const creating = useTask<Created>()
  const encryptable = format === 'zip' || format === 'sevenZ'

  const add = (paths: string[]) => setSources((prev) => [...prev, ...paths.filter((p) => !prev.includes(p))])
  // 每次拖入都是新的数组，据此在渲染时把它加入列表
  const [seenDrop, setSeenDrop] = useState(dropped)
  if (dropped !== seenDrop) {
    setSeenDrop(dropped)
    add(dropped)
  }

  // 源或格式变化时重新推荐输出文件（用户手动选择的除外）
  const custom = output?.custom ?? false
  useEffect(() => {
    if (custom || sources.length === 0) return
    let alive = true
    call<string>('suggest', { sources, format })
      .then((path) => alive && setOutput({ path, custom: false }))
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [sources, format, custom, call])

  useEffect(() => {
    if (creating.status === 'succeeded' && creating.result) toast.success(t('create.done', { size: formatBytes(creating.result.size) }))
    if (creating.status === 'failed' && creating.error) toast.error(errorMessage(creating.error))
  }, [creating.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const guard = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickFiles = () => guard(async () => add(await host.dialog.openFiles()))
  const pickFolder = () =>
    guard(async () => {
      const dir = await host.dialog.openDirectory()
      if (dir) add([dir])
    })
  const pickOutput = () =>
    guard(async () => {
      const path = await host.dialog.saveFile(output?.path, [{ name: EXTENSION[format], extensions: [EXTENSION[format].split('.').pop()!] }])
      if (path) setOutput({ path, custom: true })
    })

  const start = () =>
    output && creating.start('create', { sources, output: output.path, format, level, password: encryptable && password ? password : null })
  const progress = creating.progress && creating.progress.total > 0 ? (creating.progress.done / creating.progress.total) * 100 : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_340px] gap-3">
      <Panel
        icon={<PackagePlus />}
        title={t('create.sources')}
        extra={sources.length > 0 && <Badge>{sources.length}</Badge>}
        actions={
          <>
            <Button size="sm" onClick={pickFiles}>
              <FilePlus2 />
              {t('create.addFiles')}
            </Button>
            <Button size="sm" onClick={pickFolder}>
              <FolderPlus />
              {t('create.addFolder')}
            </Button>
          </>
        }
        bodyClassName="overflow-auto p-2"
      >
        {sources.length === 0 ? (
          <Empty icon={<PackagePlus />} title={t('create.empty')} description={t('create.emptyHint')} />
        ) : (
          <ul className="space-y-0.5">
            {sources.map((path) => (
              <li key={path} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                <span className="min-w-0 flex-1 truncate text-[13px] text-fg" title={path}>
                  {path}
                </span>
                <Button size="icon-sm" variant="ghost" aria-label={t('create.remove')} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={() => setSources(sources.filter((p) => p !== path))}>
                  <X />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title={t('create.options')} bodyClassName="space-y-4 overflow-auto p-3">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('create.format')}</p>
          <SegmentedControl<Output>
            size="sm"
            value={format}
            onValueChange={(value) => {
              setFormat(value)
              if (output?.custom) setOutput({ path: output.path.replace(/\.(zip|7z|tar\.gz|tar\.xz)$/i, `.${EXTENSION[value]}`), custom: true })
            }}
            aria-label={t('create.format')}
            options={(['zip', 'sevenZ', 'tarGz', 'tarXz'] as Output[]).map((value) => ({ value, label: EXTENSION[value] === 'zip' ? 'ZIP' : EXTENSION[value] }))}
          />
          <p className="text-[11px] leading-snug text-fg-subtle">{t(`create.formatHint.${format}`)}</p>
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('create.level')}</p>
          <Select<number> size="sm" value={level} onValueChange={setLevel} aria-label={t('create.level')} options={LEVELS.map((value) => ({ value, label: t(`levels.${value}`) }))} />
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('create.password')}</p>
          <Tooltip content={encryptable ? t('create.passwordHint') : t('create.passwordUnsupported')}>
            <Input type="password" value={encryptable ? password : ''} onChange={(e) => setPassword(e.target.value)} disabled={!encryptable} placeholder={t('create.passwordPlaceholder')} aria-label={t('create.password')} />
          </Tooltip>
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('create.output')}</p>
          <Button block className="justify-start" onClick={pickOutput} disabled={sources.length === 0}>
            <Save />
            <span className="truncate">{output?.path ?? t('create.outputEmpty')}</span>
          </Button>
        </div>
        {creating.running && <Progress value={progress} />}
        <Button block variant="primary" loading={creating.running} disabled={sources.length === 0 || !output} onClick={start}>
          <PackagePlus />
          {t('create.start')}
        </Button>
        {creating.status === 'succeeded' && creating.result && (
          <div className="space-y-2 rounded-control bg-success-soft p-2.5 text-xs text-success">
            <p>{t('create.summary', { files: creating.result.files, bytes: formatBytes(creating.result.bytes), size: formatBytes(creating.result.size) })}</p>
            {creating.result.skipped > 0 && <p className="text-fg-muted">{t('create.skipped', { count: creating.result.skipped })}</p>}
            <Button size="sm" onClick={() => host.revealPath(creating.result!.output).catch((error) => toast.error(errorMessage(error)))}>
              <FolderSearch />
              {t('create.show')}
            </Button>
          </div>
        )}
      </Panel>
    </div>
  )
}
