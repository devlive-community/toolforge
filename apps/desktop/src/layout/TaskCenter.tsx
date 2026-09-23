import { useEffect, useState } from 'react'
import { Badge, Button, Drawer, Empty, LogViewer, Progress, cn, type LogEntry } from '@toolforge/ui'
import { cancelTask, taskLogs, useErrorMessage, useLogFormatter, type TaskRecord } from '@toolforge/plugin-ui-sdk'
import { ChevronRight, ListChecks, Square } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { pluginById, useApp } from '../stores/app'
import { useTasks } from '../stores/tasks'

const statusVariant = {
  running: 'info',
  succeeded: 'success',
  failed: 'danger',
  cancelled: 'neutral',
  interrupted: 'warning',
} as const

/** 运行中任务的日志每秒从落盘文件刷新一次 */
const LOG_POLL_MS = 1000

function TaskLogs({ task }: { task: TaskRecord }) {
  const { t } = useTranslation()
  const format = useLogFormatter([task.pluginId, 'common'])
  const [entries, setEntries] = useState<LogEntry[]>([])

  useEffect(() => {
    let alive = true
    const load = () =>
      taskLogs(task.id)
        .then((lines) => {
          if (alive) setEntries(lines.map((line, i) => ({ id: i, ts: line.ts, level: line.level, text: format(line) })))
        })
        .catch(() => {})
    load()
    if (task.status !== 'running') return () => void (alive = false)
    const timer = setInterval(load, LOG_POLL_MS)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [task.id, task.status, format])

  return (
    <div className="h-64 border-t border-border">
      <LogViewer entries={entries} emptyText={t('tasks.noLogs')} jumpLabel={t('tasks.jumpLatest')} />
    </div>
  )
}

function TaskItem({ task }: { task: TaskRecord }) {
  const { t, i18n } = useTranslation()
  const text = usePluginText()
  const errorMessage = useErrorMessage(task.pluginId)
  const manifest = useApp((s) => pluginById(s.plugins, task.pluginId))
  const live = useTasks((s) => s.live[task.id])
  const [expanded, setExpanded] = useState(false)
  const running = task.status === 'running'
  const fnKey = `functions.${task.function}`
  const fnLabel = manifest && i18n.exists(fnKey, { ns: manifest.id }) ? t(fnKey, { ns: manifest.id }) : task.function
  const percent = live && live.total > 0 ? (live.done / live.total) * 100 : null

  return (
    <li className="border-b border-border">
      <div className="flex items-start gap-3 px-4 py-3">
        <button
          type="button"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
          className="flex min-w-0 flex-1 items-start gap-2 rounded-control text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ChevronRight className={cn('mt-0.5 size-4 shrink-0 text-fg-subtle transition-transform', expanded && 'rotate-90')} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <p className="truncate text-[13px] font-medium text-fg">
                {manifest ? text(manifest, manifest.name) : task.pluginId} · {fnLabel}
              </p>
              <Badge variant={statusVariant[task.status]}>{t(`tasks.status.${task.status}`)}</Badge>
            </div>
            <p className="mt-0.5 text-xs text-fg-muted">
              {new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'medium' }).format(task.startedAt)}
              {task.elapsedMs !== null && ` · ${t('tasks.elapsed', { ms: task.elapsedMs })}`}
            </p>
            {running && (
              <div className="mt-2 space-y-1">
                <Progress value={percent} size="sm" />
                {percent !== null && <p className="text-xs text-fg-muted tabular-nums">{Math.floor(percent)}%</p>}
              </div>
            )}
            {task.errorCode && <p className="mt-1 text-xs text-danger">{errorMessage({ code: task.errorCode })}</p>}
          </div>
        </button>
        {running && (
          <Button size="sm" onClick={() => cancelTask(task.id)}>
            <Square />
            {t('tasks.cancel')}
          </Button>
        )}
      </div>
      {expanded && <TaskLogs task={task} />}
    </li>
  )
}

export function TaskCenter() {
  const { t } = useTranslation()
  const { open, setOpen, records } = useTasks()

  return (
    <Drawer open={open} onOpenChange={setOpen} title={t('tasks.title')} closeLabel={t('common:close')}>
      {records.length === 0 ? (
        <Empty icon={<ListChecks />} title={t('tasks.empty')} description={t('tasks.emptyHint')} />
      ) : (
        <ul>
          {records.map((task) => (
            <TaskItem key={task.id} task={task} />
          ))}
        </ul>
      )}
    </Drawer>
  )
}
