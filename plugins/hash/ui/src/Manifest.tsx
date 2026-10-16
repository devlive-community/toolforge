import { useState } from 'react'
import { Badge, Button, Empty, LogViewer, Panel, Progress, SegmentedControl, Select, Switch, cn, toast } from '@toolforge/ui'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileCheck2, FolderOpen, ListChecks, Play, ScrollText, Square } from 'lucide-react'
import { formatBytes } from './format'
import { ALGORITHM_LABELS, MANIFEST_ALGORITHMS, MANIFEST_EXTENSIONS, type ManifestAlgorithm, type ManifestReport, type ManifestStatus } from './types'

type Mode = 'verify' | 'create'

const baseName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path
const STATUS_VARIANT: Record<ManifestStatus, 'success' | 'danger' | 'warning'> = { ok: 'success', failed: 'danger', missing: 'warning', error: 'danger' }

export function Manifest() {
  const { t, errorMessage } = usePlugin()
  const task = useTask<ManifestReport>()
  const [mode, setMode] = useState<Mode>('verify')
  const [dir, setDir] = useState<string | null>(null)
  const [manifest, setManifest] = useState<string | null>(null)
  const [algorithm, setAlgorithm] = useState<ManifestAlgorithm>('sha256')
  const [verifyAlgorithm, setVerifyAlgorithm] = useState<ManifestAlgorithm | 'auto'>('auto')
  const [recursive, setRecursive] = useState(true)

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const pickDir = () => run(async () => setDir(await host.dialog.openDirectory()))
  const pickManifest = () => run(async () => setManifest(await host.dialog.openFile()))

  const start = () =>
    run(async () => {
      if (mode === 'verify') {
        if (!manifest) return
        await task.start('manifest_verify', { manifest, algorithm: verifyAlgorithm === 'auto' ? undefined : verifyAlgorithm })
        return
      }
      if (!dir) return
      const extension = MANIFEST_EXTENSIONS[algorithm]
      const output = await host.dialog.saveFile(`${dir}/${baseName(dir)}.${extension}`, [{ name: t('manifest.filter'), extensions: [extension, 'txt'] }])
      if (!output) return
      await task.start('manifest_create', { dir, algorithm, recursive, output })
    })

  const report = task.result
  const percent = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const ready = mode === 'verify' ? !!manifest : !!dir
  const algorithmOptions = MANIFEST_ALGORITHMS.map((value) => ({ value, label: ALGORITHM_LABELS[value] }))

  return (
    <div className="grid h-full grid-cols-[minmax(300px,0.75fr)_minmax(0,1.6fr)] gap-3">
      <Panel icon={<ListChecks />} title={t('manifest.title')} bodyClassName="flex flex-col">
        <div className="min-h-0 flex-1 space-y-4 overflow-auto p-4">
          <SegmentedControl<Mode>
            value={mode}
            onValueChange={setMode}
            className="flex w-full [&>button]:flex-1"
            aria-label={t('manifest.title')}
            options={[
              { value: 'verify', label: t('manifest.verify') },
              { value: 'create', label: t('manifest.create') },
            ]}
          />
          {mode === 'verify' ? (
            <>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('manifest.file')}</p>
                <Button block onClick={pickManifest} disabled={task.running} className="justify-start">
                  <FileCheck2 />
                  <span className="truncate">{manifest ? baseName(manifest) : t('manifest.pickFile')}</span>
                </Button>
                {manifest && <p className="truncate text-[11px] text-fg-subtle" data-selectable>{manifest}</p>}
              </div>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('manifest.algorithm')}</p>
                <Select<ManifestAlgorithm | 'auto'>
                  value={verifyAlgorithm}
                  onValueChange={setVerifyAlgorithm}
                  className="w-full"
                  aria-label={t('manifest.algorithm')}
                  options={[{ value: 'auto', label: t('manifest.auto') }, ...algorithmOptions]}
                />
              </div>
              <p className="text-[11px] leading-relaxed text-fg-subtle">{t('manifest.verifyHint')}</p>
            </>
          ) : (
            <>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('manifest.dir')}</p>
                <Button block onClick={pickDir} disabled={task.running} className="justify-start">
                  <FolderOpen />
                  <span className="truncate">{dir ? baseName(dir) : t('manifest.pickDir')}</span>
                </Button>
                {dir && <p className="truncate text-[11px] text-fg-subtle" data-selectable>{dir}</p>}
              </div>
              <div className="space-y-1.5">
                <p className="text-xs font-medium text-fg-muted">{t('manifest.algorithm')}</p>
                <Select<ManifestAlgorithm> value={algorithm} onValueChange={setAlgorithm} className="w-full" aria-label={t('manifest.algorithm')} options={algorithmOptions} />
              </div>
              <label className="flex items-center justify-between rounded-control border border-border px-3 py-2 text-[13px] text-fg">
                {t('manifest.recursive')}
                <Switch size="sm" checked={recursive} onCheckedChange={setRecursive} aria-label={t('manifest.recursive')} />
              </label>
              <p className="text-[11px] leading-relaxed text-fg-subtle">{t('manifest.createHint')}</p>
            </>
          )}
        </div>
        <div className="shrink-0 space-y-2.5 border-t border-border p-3">
          {task.running && (
            <div className="space-y-1.5">
              <Progress value={percent} />
              <p className="flex justify-between text-xs text-fg-muted tabular-nums">
                <span>{task.stage ? t(`stages.${task.stage}`, { defaultValue: task.stage }) : t('files.preparing')}</span>
                {task.progress && (
                  <span>
                    {formatBytes(task.progress.done)} / {formatBytes(task.progress.total)}
                  </span>
                )}
              </p>
            </div>
          )}
          {task.running ? (
            <Button block onClick={task.cancel}>
              <Square />
              {t('files.cancel')}
            </Button>
          ) : (
            <Button block variant="primary" disabled={!ready} onClick={() => void start()}>
              <Play />
              {t(mode === 'verify' ? 'manifest.startVerify' : 'manifest.startCreate')}
            </Button>
          )}
        </div>
      </Panel>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(160px,0.6fr)] gap-3">
        <Panel
          icon={<ListChecks />}
          title={t('manifest.result')}
          extra={
            report && (
              <div className="flex flex-wrap items-center gap-1.5">
                <Badge variant="success">{t('manifest.status.ok')} {report.summary.ok}</Badge>
                {report.summary.failed > 0 && <Badge variant="danger">{t('manifest.status.failed')} {report.summary.failed}</Badge>}
                {report.summary.missing > 0 && <Badge variant="warning">{t('manifest.status.missing')} {report.summary.missing}</Badge>}
                {report.summary.errors > 0 && <Badge variant="danger">{t('manifest.status.error')} {report.summary.errors}</Badge>}
              </div>
            )
          }
          footer={
            report && (
              <>
                <span>{ALGORITHM_LABELS[report.algorithm]}</span>
                <span>{formatBytes(report.totalBytes)}</span>
                {report.output && <span className="truncate" data-selectable>{t('manifest.saved', { path: report.output })}</span>}
                {report.summary.invalidLines.length > 0 && <span className="text-warning">{t('manifest.invalidLines', { lines: report.summary.invalidLines.slice(0, 10).join(', ') })}</span>}
                <span className="ml-auto">{t('manifest.elapsed', { ms: report.elapsedMs })}</span>
              </>
            )
          }
          bodyClassName="overflow-auto p-2"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : task.status === 'cancelled' ? (
            <Empty icon={<ListChecks />} title={t('files.cancelled')} />
          ) : !report ? (
            <Empty icon={<ListChecks />} title={task.running ? t('files.running') : t('manifest.empty')} description={task.running ? undefined : t('manifest.emptyHint')} />
          ) : (
            <ul className="space-y-1">
              {report.entries.map((entry) => (
                <li key={entry.path} className={cn('rounded-control px-2.5 py-1.5 hover:bg-hover', entry.status !== 'ok' && 'bg-surface-2')}>
                  <div className="flex items-center gap-2">
                    <Badge variant={STATUS_VARIANT[entry.status]}>{t(`manifest.status.${entry.status}`)}</Badge>
                    <code className="min-w-0 flex-1 truncate font-mono text-xs text-fg" data-selectable>
                      {entry.path}
                    </code>
                  </div>
                  {entry.status === 'failed' && (
                    <div className="mt-1 space-y-0.5 pl-1 font-mono text-[11px] break-all" data-selectable>
                      <p className="text-fg-subtle">{t('manifest.expected')} {entry.expected}</p>
                      <p className="text-danger">{t('manifest.actual')} {entry.actual}</p>
                    </div>
                  )}
                  {entry.error && <p className="mt-1 pl-1 text-[11px] text-danger">{errorMessage(entry.error)}</p>}
                </li>
              ))}
              {report.truncated && <li className="px-2.5 py-1.5 text-xs text-fg-subtle">{t('manifest.truncated')}</li>}
            </ul>
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('files.logs')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('files.noLogs')} jumpLabel={t('files.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}
