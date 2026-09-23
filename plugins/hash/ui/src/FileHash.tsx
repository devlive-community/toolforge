import { useEffect, useState } from 'react'
import { Badge, Button, Empty, LogViewer, Panel, Progress, Tooltip, cn, toast } from '@toolforge/ui'
import { ValueRow, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FilePlus2, Files, Play, ScrollText, Square, TriangleAlert, Upload, X } from 'lucide-react'
import { formatBytes } from './format'
import { ALGORITHM_LABELS, type Algorithm, type FilesReport } from './types'

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path

export function FileHash({ algorithms, uppercase }: { algorithms: Algorithm[]; uppercase: boolean }) {
  const { t, errorMessage } = usePlugin()
  const task = useTask<FilesReport>()
  const [paths, setPaths] = useState<string[]>([])
  const [hovering, setHovering] = useState(false)

  const addPaths = (next: string[]) => setPaths((prev) => [...prev, ...next.filter((p) => !prev.includes(p))])

  useEffect(() => {
    const off = host.onFileDrop({ drop: addPaths, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const pick = async () => {
    try {
      addPaths(await host.dialog.openFiles())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const percent = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const canStart = paths.length > 0 && algorithms.length > 0 && !task.running

  return (
    <div className="grid h-full grid-cols-[minmax(300px,0.75fr)_minmax(0,1.6fr)] gap-3">
      <Panel
        icon={<Files />}
        title={t('files.title')}
        extra={paths.length > 0 && <Badge>{paths.length}</Badge>}
        actions={
          <>
            <Button size="sm" onClick={pick} disabled={task.running}>
              <FilePlus2 />
              {t('files.add')}
            </Button>
            <Button size="sm" onClick={() => setPaths([])} disabled={task.running || paths.length === 0}>
              {t('common:clear')}
            </Button>
          </>
        }
        bodyClassName="flex flex-col"
      >
        <div className="min-h-0 flex-1 overflow-auto p-2">
          {paths.length === 0 ? (
            <button
              type="button"
              onClick={pick}
              className={cn(
                'flex h-full w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <Upload className="size-6" />
              <span className="text-[13px] font-medium">{t('files.drop')}</span>
              <span className="text-xs text-fg-subtle">{t('files.dropHint')}</span>
            </button>
          ) : (
            <ul className={cn('space-y-1 rounded-card', hovering && 'ring-2 ring-primary')}>
              {paths.map((path) => (
                <li key={path} className="group flex items-center gap-2 rounded-control px-2.5 py-2 hover:bg-hover">
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] text-fg">{baseName(path)}</p>
                    <p className="truncate text-xs text-fg-subtle" data-selectable>
                      {path}
                    </p>
                  </div>
                  <Tooltip content={t('files.remove')}>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={t('files.remove')}
                      disabled={task.running}
                      className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                      onClick={() => setPaths((prev) => prev.filter((p) => p !== path))}
                    >
                      <X />
                    </Button>
                  </Tooltip>
                </li>
              ))}
            </ul>
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
                    {percent !== null && ` · ${Math.floor(percent)}%`}
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
            <Button block variant="primary" disabled={!canStart} onClick={() => task.start('hash_files', { paths, algorithms, uppercase })}>
              <Play />
              {t('files.start')}
            </Button>
          )}
        </div>
      </Panel>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_minmax(180px,0.7fr)] gap-3">
        <Panel
          icon={<Files />}
          title={t('files.result')}
          extra={
            task.status === 'succeeded' && task.result ? (
              <Badge variant="success">{t('files.summary', { count: task.result.files.length, size: formatBytes(task.result.totalBytes), ms: task.result.elapsedMs })}</Badge>
            ) : task.status === 'cancelled' ? (
              <Badge>{t('files.cancelled')}</Badge>
            ) : null
          }
          bodyClassName="overflow-auto p-2"
        >
          {task.status === 'failed' && task.error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
          ) : !task.result ? (
            <Empty icon={<Files />} title={task.running ? t('files.running') : t('files.empty')} description={task.running ? undefined : t('files.emptyHint')} />
          ) : (
            <div className="space-y-2">
              {task.result.files.map((file) => (
                <section key={file.path} className="rounded-card border border-border">
                  <header className="flex items-center gap-2 border-b border-border px-3 py-2">
                    <p className="truncate text-[13px] font-medium text-fg">{file.name}</p>
                    <span className="shrink-0 text-xs text-fg-subtle">{formatBytes(file.size)}</span>
                  </header>
                  {file.error ? (
                    <p className="flex items-center gap-2 px-3 py-2 text-xs text-danger">
                      <TriangleAlert className="size-3.5" />
                      {errorMessage(file.error)}
                    </p>
                  ) : (
                    <div className="py-1">
                      {Object.entries(file.digests).map(([algorithm, digest]) => (
                        <ValueRow key={algorithm} label={ALGORITHM_LABELS[algorithm as Algorithm]} value={digest ?? ''} />
                      ))}
                    </div>
                  )}
                </section>
              ))}
            </div>
          )}
        </Panel>
        <Panel icon={<ScrollText />} title={t('files.logs')} extra={task.logs.length > 0 && <Badge>{task.logs.length}</Badge>}>
          <LogViewer entries={task.logs} emptyText={t('files.noLogs')} jumpLabel={t('files.jumpLatest')} />
        </Panel>
      </div>
    </div>
  )
}
