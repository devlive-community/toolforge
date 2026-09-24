import { useCallback, useEffect, useState } from 'react'
import { Badge, Button, Progress, Tooltip, cn, toast } from '@toolforge/ui'
import { invoke } from '@tauri-apps/api/core'
import { Check, Download, Pause, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePlugin } from './context'
import { useErrorMessage } from './i18n'
import { startResourceDownload } from './tasks'
import { toAppError } from './types'
import { useTask } from './useTask'

/** 与 Rust `ResourceStatus` 对应 */
export interface ResourceStatus {
  id: string
  size: number
  license: string | null
  installed: boolean
  /** 已下载但未完成的字节数，可续传 */
  partial: number
  installedAt: number | null
}

/** 字节数展示（纯展示格式化） */
export function formatBytes(bytes: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value)} ${units[unit]}`
}

const wrap = async <T,>(promise: Promise<T>) => {
  try {
    return await promise
  } catch (error) {
    throw toAppError(error)
  }
}

/**
 * 当前插件在 manifest 中声明的资源：查询状态、下载（任务，实时进度与日志）、删除。
 * 同一时间只下载一个资源。
 */
export function useResources() {
  const { manifest } = usePlugin()
  const task = useTask()
  const [items, setItems] = useState<ResourceStatus[] | null>(null)
  /** 最近一次下载的资源 id（用于在对应卡片上展示进度与错误） */
  const [target, setTarget] = useState<string | null>(null)

  const [version, setVersion] = useState(0)
  const refresh = useCallback(() => setVersion((v) => v + 1), [])

  // 首次加载、手动刷新、下载开始或结束（成功、失败、取消）时重新读取状态
  useEffect(() => {
    let alive = true
    invoke<ResourceStatus[]>('resource_list', { pluginId: manifest.id }).then(
      (list) => alive && setItems(list),
      () => alive && setItems([]),
    )
    return () => {
      alive = false
    }
  }, [manifest.id, version, task.status, task.taskId])

  const download = useCallback(
    (id: string) => {
      setTarget(id)
      return task.startWith((listener) => startResourceDownload(manifest.id, id, listener))
    },
    [manifest.id, task],
  )

  const remove = useCallback(
    async (id: string) => {
      await wrap(invoke<void>('resource_delete', { pluginId: manifest.id, resourceId: id }))
      refresh()
    },
    [manifest.id, refresh],
  )

  const get = useCallback((id: string) => items?.find((item) => item.id === id) ?? null, [items])

  return { items, get, refresh, download, remove, task, target, downloading: task.running ? target : null }
}

export type ResourcesApi = ReturnType<typeof useResources>

export interface ResourceItemProps {
  resources: ResourcesApi
  id: string
  /** 是否为当前选中的资源（高亮） */
  selected?: boolean
  onSelect?: () => void
  className?: string
}

/**
 * 资源卡片：名称与说明取插件命名空间下的 `resources.<id>.name` / `resources.<id>.description`，
 * 展示大小、许可证、下载进度，并提供下载 / 续传 / 暂停 / 删除操作。
 */
export function ResourceItem({ resources, id, selected, onSelect, className }: ResourceItemProps) {
  const { t } = usePlugin()
  const { t: tc } = useTranslation('common')
  const errorMessage = useErrorMessage()
  const status = resources.get(id)
  const [confirming, setConfirming] = useState(false)
  const active = resources.downloading === id
  const { task } = resources
  const percent = active && task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const failed = resources.target === id && task.status === 'failed' ? task.error : null

  useEffect(() => {
    if (!confirming) return
    const timer = setTimeout(() => setConfirming(false), 3000)
    return () => clearTimeout(timer)
  }, [confirming])

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const lastLog = active ? task.logs[task.logs.length - 1]?.text : null

  return (
    <div
      role={onSelect ? 'button' : undefined}
      tabIndex={onSelect ? 0 : undefined}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (onSelect && (event.key === 'Enter' || event.key === ' ')) {
          event.preventDefault()
          onSelect()
        }
      }}
      className={cn(
        'space-y-2 rounded-card border p-3 text-left outline-none transition-colors',
        onSelect && 'cursor-pointer focus-visible:ring-2 focus-visible:ring-ring',
        selected ? 'border-primary bg-primary-soft/40' : 'border-border hover:bg-hover',
        className,
      )}
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="flex flex-wrap items-center gap-1.5 text-[13px] font-medium text-fg">
            {t(`resources.${id}.name`)}
            {status?.installed && (
              <Badge variant="success">
                <Check className="size-3" />
                {tc('resources.installed')}
              </Badge>
            )}
          </p>
          <p className="mt-0.5 text-xs text-fg-muted">{t(`resources.${id}.description`)}</p>
          {status && (
            <p className="mt-1 text-[11px] text-fg-subtle">
              {formatBytes(status.size)}
              {status.license && ` · ${status.license}`}
              {!status.installed && status.partial > 0 && !active && ` · ${tc('resources.partial', { size: formatBytes(status.partial) })}`}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1" onClick={(event) => event.stopPropagation()}>
          {active ? (
            <Tooltip content={tc('resources.pause')}>
              <Button size="icon-sm" aria-label={tc('resources.pause')} onClick={() => run(task.cancel)}>
                <Pause />
              </Button>
            </Tooltip>
          ) : status?.installed ? (
            <Button
              size="sm"
              variant={confirming ? 'danger' : 'ghost'}
              disabled={resources.downloading !== null}
              onClick={() => (confirming ? run(() => resources.remove(id)).then(() => setConfirming(false)) : setConfirming(true))}
            >
              <Trash2 />
              {confirming ? tc('resources.confirmDelete') : tc('resources.delete')}
            </Button>
          ) : (
            <Button size="sm" variant="primary" disabled={!status || resources.downloading !== null} onClick={() => run(() => resources.download(id))}>
              <Download />
              {status && status.partial > 0 ? tc('resources.resume') : tc('resources.download')}
            </Button>
          )}
        </div>
      </div>
      {active && (
        <div className="space-y-1">
          <Progress value={percent} />
          <p className="flex justify-between gap-3 text-[11px] text-fg-muted tabular-nums">
            <span className="truncate">{lastLog ?? tc('resources.connecting')}</span>
            {task.progress && (
              <span className="shrink-0">{`${formatBytes(task.progress.done)} / ${formatBytes(task.progress.total)}`}</span>
            )}
          </p>
        </div>
      )}
      {failed && !status?.installed && <p className="text-xs text-danger">{errorMessage(failed)}</p>}
    </div>
  )
}
