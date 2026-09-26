import { useEffect, useEffectEvent, useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Panel, Progress, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, ResourceItem, host, useCopy, usePlugin, useResources, useTask } from '@toolforge/plugin-ui-sdk'
import { Brain, ClipboardPaste, Download, ImagePlus, ScanText, Square, TextSelect, Upload } from 'lucide-react'
import type { Output, Source } from './types'

const MODELS = ['ppocr-v4-det', 'ppocr-v4-rec']
const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff']
const isImage = (path: string) => IMAGE_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')

export function OcrTool() {
  const { t, errorMessage } = usePlugin()
  const resources = useResources()
  const task = useTask<Output>()
  const { copy } = useCopy()
  // 用户编辑后的文字，与所属任务绑定；未编辑时直接展示识别结果
  const [edited, setEdited] = useState<{ taskId: string | null; text: string } | null>(null)
  const [hovering, setHovering] = useState(false)
  const [showBoxes, setShowBoxes] = useState(true)
  const [active, setActive] = useState<number | null>(null)
  const [downloadAll, setDownloadAll] = useState(false)

  const missing = MODELS.filter((id) => !resources.get(id)?.installed)
  const ready = resources.items !== null && missing.length === 0
  const result = task.result
  const text = edited && edited.taskId === task.taskId ? edited.text : (result?.text ?? '')

  // 依次下载缺失的模型（资源下载同一时间只能进行一个）；全部完成或失败后停止
  if (downloadAll && !resources.downloading && (missing.length === 0 || resources.task.status === 'failed')) {
    setDownloadAll(false)
  }
  const nextMissing = downloadAll && !resources.downloading ? missing[0] : undefined
  useEffect(() => {
    if (nextMissing) resources.download(nextMissing)
  }, [nextMissing, resources])

  const recognize = (source: Source) => {
    if (!ready) {
      toast.error(t('model.required'))
      return
    }
    setActive(null)
    task.start('recognize', { source })
  }

  useEffect(() => {
    if (task.status === 'failed' && task.error && task.error.code !== 'task.cancelled') toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const onDrop = useEffectEvent((paths: string[]) => {
    const path = paths.find(isImage)
    if (path) recognize({ kind: 'file', path })
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: onDrop, over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  // ⌘V / Ctrl+V：识别剪贴板中的截图（编辑识别结果时不拦截）
  const onPaste = useEffectEvent(() => recognize({ kind: 'clipboard' }))
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const editing = (event.target as HTMLElement | null)?.closest('input, textarea, [contenteditable="true"]')
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'v' && !editing) {
        event.preventDefault()
        onPaste()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('source.filter'), extensions: IMAGE_EXTENSIONS }])
      if (path) recognize({ kind: 'file', path })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const progress = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null
  const confidence = result && result.lines.length > 0 ? result.lines.reduce((sum, line) => sum + line.score, 0) / result.lines.length : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(260px,300px)_minmax(0,1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <Panel icon={<Brain />} title={t('model.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          {MODELS.map((id) => (
            <ResourceItem key={id} resources={resources} id={id} />
          ))}
          {resources.items !== null && missing.length > 0 && (
            <Button block variant="primary" disabled={Boolean(resources.downloading) || downloadAll} onClick={() => setDownloadAll(true)}>
              <Download />
              {t('model.downloadAll')}
            </Button>
          )}
          <p className="text-[11px] text-fg-subtle">{t('model.hint')}</p>
        </Panel>

        <Panel icon={<ScanText />} title={t('source.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          <button
            type="button"
            onClick={pick}
            disabled={task.running}
            className={cn(
              'flex h-36 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
              'focus-visible:ring-2 focus-visible:ring-ring',
              hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
            )}
          >
            <Upload className="size-5" />
            <span className="text-[13px] font-medium">{t('source.drop')}</span>
            <span className="text-xs text-fg-subtle">{t('source.dropHint')}</span>
          </button>
          <div className="grid grid-cols-2 gap-2">
            <Button onClick={pick} disabled={task.running}>
              <ImagePlus />
              {t('source.open')}
            </Button>
            <Button onClick={() => recognize({ kind: 'clipboard' })} disabled={task.running}>
              <ClipboardPaste />
              {t('source.paste')}
            </Button>
          </div>
          {task.running && (
            <div className="space-y-1.5 pt-1">
              <Progress value={task.stage === 'ocr.recognize' ? progress : null} />
              <div className="flex items-center justify-between gap-2">
                <p className="text-xs text-fg-muted tabular-nums">
                  {task.stage === 'ocr.recognize' && task.progress
                    ? t('stages.lines', { done: task.progress.done, total: task.progress.total })
                    : t(`stages.${task.stage ?? 'ocr.load'}`)}
                </p>
                <Button size="sm" variant="ghost" onClick={task.cancel}>
                  <Square />
                  {t('source.cancel')}
                </Button>
              </div>
            </div>
          )}
          {!ready && resources.items !== null && <p className="text-[11px] text-warning">{t('model.required')}</p>}
        </Panel>
      </div>

      {result ? (
        <div className="grid min-h-0 grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)] gap-3">
          <Panel
            icon={<ScanText />}
            title={<span className="truncate">{result.name ?? t('result.clipboard')}</span>}
            extra={<Badge>{t('result.size', { width: result.width, height: result.height })}</Badge>}
            actions={
              <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                <Switch size="sm" checked={showBoxes} onCheckedChange={setShowBoxes} aria-label={t('result.boxes')} />
                {t('result.boxes')}
              </label>
            }
            bodyClassName="flex items-center justify-center overflow-auto bg-surface-2 p-3"
          >
            <div className="relative max-h-full max-w-full">
              <img src={result.preview} alt={result.name ?? ''} className="block max-h-[calc(100vh-16rem)] max-w-full object-contain" />
              {showBoxes && (
                <svg className="absolute inset-0 size-full" viewBox={`0 0 ${result.width} ${result.height}`} preserveAspectRatio="none">
                  {result.lines.map((line, index) => (
                    <rect
                      key={index}
                      x={line.x}
                      y={line.y}
                      width={line.w}
                      height={line.h}
                      rx={Math.min(line.h / 6, 4)}
                      onMouseEnter={() => setActive(index)}
                      onMouseLeave={() => setActive(null)}
                      onClick={() => copy(line.text)}
                      className={cn(
                        'cursor-pointer fill-primary/10 stroke-primary transition-colors [vector-effect:non-scaling-stroke]',
                        active === index ? 'fill-primary/30 stroke-2' : 'stroke-1',
                      )}
                    >
                      <title>{line.text}</title>
                    </rect>
                  ))}
                </svg>
              )}
            </div>
          </Panel>
          <Panel
            icon={<TextSelect />}
            title={t('result.text')}
            extra={
              <span className="flex items-center gap-1.5">
                <Badge>{t('result.lines', { count: result.lines.length })}</Badge>
                {confidence !== null && (
                  <Tooltip content={t('result.confidenceHint')}>
                    <Badge variant={confidence > 0.9 ? 'success' : 'warning'}>{t('result.confidence', { value: Math.round(confidence * 100) })}</Badge>
                  </Tooltip>
                )}
              </span>
            }
            actions={<CopyButton text={text} label={t('common:copy')} variant="outline" disabled={!text} />}
            footer={<span className="tabular-nums">{t('result.elapsed', { ms: result.elapsedMs })}</span>}
          >
            {result.lines.length === 0 ? (
              <Empty icon={<TextSelect />} title={t('result.empty')} />
            ) : (
              <CodeEditor value={text} onChange={(value) => setEdited({ taskId: task.taskId, text: value })} language="text" lineWrapping aria-label={t('result.text')} />
            )}
          </Panel>
        </div>
      ) : (
        <Panel icon={<TextSelect />} title={t('result.text')}>
          <Empty icon={<ScanText />} title={t('result.placeholder')} description={t('result.placeholderHint')} />
        </Panel>
      )}
    </div>
  )
}
