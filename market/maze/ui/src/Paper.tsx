import { useEffect, useRef, useState, useMemo } from 'react'
import { host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Button, Empty, Pagination, Select, Spinner, toast } from '@toolforge/ui'
import { Download, Eye, FileImage, Square } from 'lucide-react'
import type { Op, Options, Preview, State } from './types'

function ImagePreview({ svg }: { svg: string }) {
  const { t } = usePlugin()
  const url = useMemo(() => URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' })), [svg])
  useEffect(() => () => URL.revokeObjectURL(url), [url])
  return <img src={url} alt={t('paper.previewAlt')} className="h-full min-h-0 w-full object-contain" />
}
export function Paper({
  disabled,
  source,
  run,
  onBusy,
}: {
  disabled: boolean
  source: { current: State | null }
  run: (op?: Op) => Promise<void>
  onBusy: (value: boolean) => void
}) {
  const { t, errorMessage } = usePlugin()
  const [options, setOptions] = useState<Options>({ count: 4, perPage: 4, sheet: 'puzzle' })
  const [preview, setPreview] = useState<Preview | null>(null)
  const task = useTask<Preview & { saved?: boolean }>()
  const pending = useRef(false)
  const handled = useRef<string | null>(null)
  const mounted = useRef(true)
  const busyCallback = useRef(onBusy)
  useEffect(() => {
    busyCallback.current = onBusy
  })
  useEffect(() => {
    mounted.current = true
    return () => {
      mounted.current = false
      busyCallback.current(false)
    }
  }, [])
  const end = () => {
    pending.current = false
    if (mounted.current) onBusy(false)
  }
  const change = (patch: Partial<Options>) => {
    setOptions({ ...options, ...patch })
    setPreview(null)
    task.reset()
  }
  const begin = async (format?: 'pdf' | 'svg' | 'png', page = 0) => {
    if (disabled || pending.current || !source.current?.game) return
    pending.current = true
    onBusy(true)
    try {
      const path = format ? await host.dialog.saveFile(`maze-${options.sheet}.${format}`, [{ name: format.toUpperCase(), extensions: [format] }]) : null
      if (format && !path) {
        end()
        return
      }
      const sheet = !format && options.sheet === 'both' ? 'puzzle' : options.sheet
      if (sheet !== 'puzzle') await run({ type: 'markAnswer' })
      if (!mounted.current) {
        end()
        return
      }
      const count = format && format !== 'pdf' ? 1 : options.count
      const id = await task.start(format ? 'export' : 'preview', { state: source.current, options: { ...options, sheet, count }, page, format, path })
      if (!id) end()
    } catch (error) {
      end()
      toast.error(errorMessage(error))
    }
  }
  useEffect(() => {
    const id = task.taskId
    if (!id || task.running || task.status === 'idle' || handled.current === id) return
    handled.current = id
    void Promise.resolve().then(() => {
      if (!mounted.current) return
      if (task.result?.svg) setPreview(task.result)
      if (task.result?.saved) toast.success(t('paper.saved'))
      pending.current = false
      busyCallback.current(false)
    })
  }, [task.taskId, task.running, task.status, task.result, t])
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-end gap-3">
        <label className="space-y-1 text-xs text-fg-muted">
          <span>{t('paper.count')}</span>
          <Select
            size="sm"
            value={options.count}
            disabled={disabled}
            options={[1, 2, 4, 8, 12].map((value) => ({ value, label: String(value) }))}
            onValueChange={(count) => change({ count })}
            aria-label={t('paper.count')}
          />
        </label>
        <label className="space-y-1 text-xs text-fg-muted">
          <span>{t('paper.perPage')}</span>
          <Select
            size="sm"
            value={options.perPage}
            disabled={disabled}
            options={[1, 2, 4].map((value) => ({ value, label: String(value) }))}
            onValueChange={(perPage) => change({ perPage })}
            aria-label={t('paper.perPage')}
          />
        </label>
        <label className="space-y-1 text-xs text-fg-muted">
          <span>{t('paper.sheet')}</span>
          <Select
            size="sm"
            value={options.sheet}
            disabled={disabled}
            options={(['puzzle', 'answer', 'both'] as const).map((value) => ({ value, label: t(`paper.${value}`) }))}
            onValueChange={(sheet) => change({ sheet })}
            aria-label={t('paper.sheet')}
          />
        </label>
        <Button size="sm" disabled={disabled} onClick={() => void begin()}>
          <Eye />
          {t('paper.preview')}
        </Button>
      </div>
      <p className="shrink-0 text-[11px] leading-relaxed text-fg-muted">{t('paper.hint')}</p>
      {task.error && (
        <p role="alert" className="text-xs text-danger">
          {errorMessage(task.error)}
        </p>
      )}
      {task.running && (
        <div className="flex shrink-0 items-center gap-2 text-xs text-fg-muted">
          <Spinner className="size-3.5" />
          {t('paper.working')}
          <Button size="sm" onClick={() => void task.cancel().catch((e) => toast.error(errorMessage(e)))}>
            <Square />
            {t('actions.cancel')}
          </Button>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-hidden rounded-card border border-border bg-surface-2 p-2">
        {preview ? (
          <ImagePreview svg={preview.svg} />
        ) : (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<FileImage />} title={t('paper.empty')} description={t('paper.noSpoiler')} />
          </div>
        )}
      </div>
      {preview && (
        <>
          <p className="shrink-0 truncate font-mono text-[10px] text-fg-subtle">{preview.ids.join(' · ')}</p>
          <Pagination
            page={preview.page + 1}
            pageSize={1}
            total={preview.total}
            onPageChange={(page) => void begin(undefined, page - 1)}
            prevLabel={t('page.previous')}
            nextLabel={t('page.next')}
            pageLabel={(page) => t('page.label', { page })}
          />
        </>
      )}
      <div className="flex shrink-0 flex-wrap gap-2">
        {(['pdf', 'svg', 'png'] as const).map((format) => (
          <Button key={format} size="sm" disabled={disabled || (format !== 'pdf' && options.sheet === 'both')} onClick={() => void begin(format)}>
            <Download />
            {t(`paper.export.${format}`)}
          </Button>
        ))}
      </div>
    </div>
  )
}
