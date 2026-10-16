import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge, Button, Empty, Input, Pagination, Switch, Tabs, Tooltip, toast } from '@toolforge/ui'
import {
  clearTasks,
  host,
  rerunTask,
  taskLogs,
  useErrorMessage,
  useLogFormatter,
  type Manifest,
  type TaskRecord,
} from '@toolforge/plugin-ui-sdk'
import { Copy, Download, History as HistoryIcon, ListChecks, RotateCcw, Search, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { pluginById, useApp } from '../stores/app'
import { TASKS_PAGE, useTasks } from '../stores/tasks'
import { Page } from './Page'

interface Entry {
  id: number
  pluginId: string
  function: string
  input: string
  output: string | null
  createdAt: number
  updatedAt: number
}

const PAGE = 50
const time = new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'short' })
const statusVariant = { running: 'info', succeeded: 'success', failed: 'danger', cancelled: 'neutral', interrupted: 'warning' } as const

/** 函数名：插件 functions.<fn> → 宿主 taskFunctions.<fn> → 原始名 */
function useFunctionLabel() {
  const { t, i18n } = useTranslation()
  return (manifest: Manifest | undefined, fn: string) => {
    const key = `functions.${fn}`
    if (manifest && i18n.exists(key, { ns: manifest.id })) return t(key, { ns: manifest.id })
    if (i18n.exists(`taskFunctions.${fn}`, { ns: 'common' })) return t(`taskFunctions.${fn}`, { ns: 'common' })
    return fn
  }
}

function Records() {
  const { t } = useTranslation()
  const text = usePluginText()
  const label = useFunctionLabel()
  const plugins = useApp((s) => s.plugins)
  const openToolWith = useApp((s) => s.openToolWith)
  const [query, setQuery] = useState('')
  const [entries, setEntries] = useState<Entry[]>([])
  const [total, setTotal] = useState(0)
  const [page, setPage] = useState(1)
  const [enabled, setEnabled] = useState(true)

  const load = useCallback(
    () =>
      invoke<{ entries: Entry[]; total: number }>('history_list', {
        query,
        limit: PAGE,
        offset: (page - 1) * PAGE,
      })
        .then((data) => {
          setEntries(data.entries)
          setTotal(data.total)
        })
        .catch(() => {}),
    [query, page],
  )

  useEffect(() => {
    const timer = setTimeout(load, 200)
    return () => clearTimeout(timer)
  }, [load])
  useEffect(() => {
    invoke<{ enabled: boolean }>('history_settings_get')
      .then((settings) => setEnabled(settings.enabled))
      .catch(() => {})
  }, [])

  const toggle = (value: boolean) => {
    setEnabled(value)
    invoke('history_settings_set', { enabled: value }).catch(() => setEnabled(!value))
  }
  const remove = (id: number) => invoke('history_delete', { id }).then(load)
  const clear = () =>
    invoke<number>('history_clear', { pluginId: null })
      .then((count) => {
        toast.success(t('history.cleared', { count }))
        return load()
      })
      .catch(() => {})
  const copy = async (value: string) => {
    await host.clipboard.writeText(value)
    toast.success(t('common:copied'))
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <Input
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setPage(1)
          }}
          placeholder={t('history.search')}
          aria-label={t('history.search')}
          leading={<Search />}
          wrapperClassName="min-w-60 flex-1"
        />
        <label className="flex items-center gap-2 text-[13px] text-fg">
          <Switch size="sm" checked={enabled} onCheckedChange={toggle} aria-label={t('history.enabled')} />
          {t('history.enabled')}
        </label>
        <Button onClick={() => void clear()} disabled={total === 0}>
          <Trash2 />
          {t('history.clear')}
        </Button>
      </div>
      <p className="text-xs text-fg-subtle">{t('history.privacy')}</p>
      {entries.length === 0 ? (
        <Empty className="py-20" icon={<HistoryIcon />} title={t(query ? 'history.noMatch' : 'history.empty')} description={query ? undefined : t('history.emptyHint')} />
      ) : (
        <ul className="space-y-2">
          {entries.map((entry) => {
            const manifest = pluginById(plugins, entry.pluginId)
            return (
              <li key={entry.id} className="group flex gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
                {manifest && <ToolTile manifest={manifest} size="sm" />}
                <div className="min-w-0 flex-1 space-y-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[13px] font-medium text-fg">{manifest ? text(manifest, manifest.name) : entry.pluginId}</span>
                    <Badge>{label(manifest, entry.function)}</Badge>
                    <span className="text-xs text-fg-subtle">{time.format(entry.updatedAt)}</span>
                  </div>
                  <p className="line-clamp-2 font-mono text-xs break-all text-fg" data-selectable>
                    {entry.input}
                  </p>
                  {entry.output && (
                    <p className="line-clamp-1 font-mono text-[11px] break-all text-fg-muted" data-selectable>
                      {t('history.output', { output: entry.output })}
                    </p>
                  )}
                </div>
                <div className="flex shrink-0 items-start gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                  {manifest && (
                    <Tooltip content={t('history.reopen')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('history.reopen')} onClick={() => openToolWith(entry.pluginId, entry.input)}>
                        <SquareArrowOutUpRight />
                      </Button>
                    </Tooltip>
                  )}
                  <Tooltip content={t('history.copyInput')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('history.copyInput')} onClick={() => void copy(entry.input)}>
                      <Copy />
                    </Button>
                  </Tooltip>
                  <Tooltip content={t('history.delete')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('history.delete')} onClick={() => void remove(entry.id)}>
                      <Trash2 />
                    </Button>
                  </Tooltip>
                </div>
              </li>
            )
          })}
        </ul>
      )}
      <Pagination
        page={page}
        pageSize={PAGE}
        total={total}
        onPageChange={setPage}
        prevLabel={t('pagination.prev')}
        nextLabel={t('pagination.next')}
        pageLabel={(n) => t('pagination.page', { page: n })}
        totalLabel={t('pagination.total', { total })}
      />
    </div>
  )
}

function TaskRow({ task }: { task: TaskRecord }) {
  const { t } = useTranslation()
  const text = usePluginText()
  const label = useFunctionLabel()
  const manifest = useApp((s) => pluginById(s.plugins, task.pluginId))
  const errorMessage = useErrorMessage(task.pluginId)
  const format = useLogFormatter([task.pluginId, 'common'])
  const setOpen = useTasks((s) => s.setOpen)

  const rerun = async () => {
    try {
      // 新任务由 task://event 的 started 事件实时前置到列表，无需重新加载
      await rerunTask(task.id)
      setOpen(true)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const exportLogs = async () => {
    try {
      const lines = await taskLogs(task.id)
      const content = lines.map((line) => `${new Date(line.ts).toISOString()} ${line.level.toUpperCase()} ${format(line)}`).join('\n')
      const path = await host.dialog.saveFile(`${task.pluginId.split('.').pop()}-${task.id}.log`, [{ name: 'Log', extensions: ['log', 'txt'] }])
      if (!path) return
      await host.fs.writeText(path, `${content}\n`)
      toast.success(t('common:saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <li className="group flex items-center gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
      {manifest && <ToolTile manifest={manifest} size="sm" />}
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-[13px] font-medium text-fg">{manifest ? text(manifest, manifest.name) : task.pluginId}</span>
          <span className="text-xs text-fg-muted">{label(manifest, task.function)}</span>
          <Badge variant={statusVariant[task.status]}>{t(`tasks.status.${task.status}`)}</Badge>
        </div>
        <p className="mt-0.5 text-xs text-fg-subtle">
          {time.format(task.startedAt)}
          {task.elapsedMs !== null && ` · ${t('tasks.elapsed', { ms: task.elapsedMs })}`}
          {task.errorCode && <span className="ml-2 text-danger">{errorMessage({ code: task.errorCode })}</span>}
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Tooltip content={t(task.rerunnable ? 'history.rerun' : 'history.notRerunnable')}>
          <Button size="icon-sm" variant="ghost" aria-label={t('history.rerun')} disabled={!task.rerunnable || task.status === 'running'} onClick={() => void rerun()}>
            <RotateCcw />
          </Button>
        </Tooltip>
        <Tooltip content={t('history.exportLogs')}>
          <Button size="icon-sm" variant="ghost" aria-label={t('history.exportLogs')} onClick={() => void exportLogs()}>
            <Download />
          </Button>
        </Tooltip>
      </div>
    </li>
  )
}

function Tasks() {
  const { t } = useTranslation()
  const records = useTasks((s) => s.records)
  const total = useTasks((s) => s.total)
  const loadPage = useTasks((s) => s.loadPage)
  const [page, setPage] = useState(1)

  useEffect(() => {
    void loadPage(1)
  }, [loadPage])

  const changePage = (next: number) => {
    setPage(next)
    void loadPage(next)
  }

  const clear = async () => {
    const count = await clearTasks()
    toast.success(t('history.tasksCleared', { count }))
    setPage(1)
    await loadPage(1)
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <p className="text-xs text-fg-subtle">{t('history.tasksHint')}</p>
        <Button onClick={() => void clear()} disabled={total === 0}>
          <Trash2 />
          {t('history.clearTasks')}
        </Button>
      </div>
      {records.length === 0 ? (
        <Empty className="py-20" icon={<ListChecks />} title={t('tasks.empty')} description={t('tasks.emptyHint')} />
      ) : (
        <ul className="space-y-2">
          {records.map((task) => (
            <TaskRow key={task.id} task={task} />
          ))}
        </ul>
      )}
      <Pagination
        page={page}
        pageSize={TASKS_PAGE}
        total={total}
        onPageChange={changePage}
        prevLabel={t('pagination.prev')}
        nextLabel={t('pagination.next')}
        pageLabel={(n) => t('pagination.page', { page: n })}
        totalLabel={t('pagination.total', { total })}
      />
    </div>
  )
}

export function History() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<'records' | 'tasks'>('records')
  return (
    <Page title={t('nav.history')} description={t('history.subtitle')}>
      <div className="mb-4">
        <Tabs
          value={tab}
          onValueChange={setTab}
          items={[
            { value: 'records', label: t('history.records'), icon: <HistoryIcon /> },
            { value: 'tasks', label: t('history.tasks'), icon: <ListChecks /> },
          ]}
        />
      </div>
      {tab === 'records' ? <Records /> : <Tasks />}
    </Page>
  )
}
