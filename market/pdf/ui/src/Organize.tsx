import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, Progress, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Copy, FilePlus2, FileText, Lock, RotateCcw, RotateCw, Save, Trash2, Undo2, Upload, X } from 'lucide-react'
import { usePdfOpener } from './usePasswordPrompt'
import { FILE_COLORS, fileName, type Metadata, type PageItem, type PdfFile } from './types'

const THUMB_WIDTH = 180
const BATCH = 6
/** 按下后移动超过这个距离才开始拖动，避免影响单击选择 */
const DRAG_THRESHOLD = 5
/** 指针离滚动区域上下边缘多近时自动滚动 */
const SCROLL_EDGE = 48

type Side = 'before' | 'after'
interface DropTarget {
  /** 为 null 表示放到最后 */
  key: string | null
  side: Side
}

let nextKey = 0
const key = () => `p${nextKey++}`

export function Organize({ dropped }: { dropped: string[] }) {
  const { t, call, errorMessage } = usePlugin()
  const { open, dialog } = usePdfOpener()
  const [files, setFiles] = useState<PdfFile[]>([])
  const [pages, setPages] = useState<PageItem[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [anchor, setAnchor] = useState<string | null>(null)
  const [thumbs, setThumbs] = useState<Record<string, string | null>>({})
  // 拖动排序用指针事件实现：Tauri 开启了文件拖放，网页里的 HTML5 拖放事件收不到
  const [dragging, setDragging] = useState<Set<string> | null>(null)
  const [target, setTarget] = useState<DropTarget | null>(null)
  const pressed = useRef<{ key: string; x: number; y: number } | null>(null)
  const justDragged = useRef(false)
  const gridRef = useRef<HTMLDivElement>(null)
  const ghostRef = useRef<HTMLDivElement>(null)
  const [metadata, setMetadata] = useState<Metadata | null>(null)
  const [writeMetadata, setWriteMetadata] = useState(false)
  const task = useTask<{ output: string; pages: number; size: number }>()
  const nextFile = useRef(0)
  const requested = useRef(new Set<string>())

  const addPaths = async (paths: string[]) => {
    for (const path of paths.filter((p) => p.toLowerCase().endsWith('.pdf'))) {
      const result = await open(path)
      if ('error' in result) {
        if (result.error) toast.error(result.error)
        continue
      }
      const id = nextFile.current++
      const file: PdfFile = { id, info: result.info, password: result.password, color: id % FILE_COLORS.length }
      setFiles((prev) => [...prev, file])
      setPages((prev) => [...prev, ...result.info.pages.map((p) => ({ key: key(), file: id, page: p.number, rotate: 0 }))])
      setMetadata((prev) => prev ?? result.info.metadata)
    }
  }
  const addRef = useRef(addPaths)
  useEffect(() => {
    addRef.current = addPaths
  })
  useEffect(() => {
    if (dropped.length > 0) void addRef.current(dropped)
  }, [dropped])

  // 按批读取缩略图，避免一次渲染太多页面
  useEffect(() => {
    const missing: { file: PdfFile; page: number }[] = []
    for (const item of pages) {
      const id = `${item.file}:${item.page}`
      if (requested.current.has(id)) continue
      const file = files.find((f) => f.id === item.file)
      if (file) missing.push({ file, page: item.page })
    }
    if (missing.length === 0) return
    const batch = missing.filter((m) => m.file.id === missing[0].file.id).slice(0, BATCH)
    batch.forEach((m) => requested.current.add(`${m.file.id}:${m.page}`))
    call<{ page: number; dataUri: string | null }[]>('thumbnails', {
      path: batch[0].file.info.path,
      password: batch[0].file.password,
      pages: batch.map((m) => m.page),
      width: THUMB_WIDTH,
    })
      .then((result) => setThumbs((prev) => ({ ...prev, ...Object.fromEntries(result.map((r) => [`${batch[0].file.id}:${r.page}`, r.dataUri])) })))
      .catch(() => setThumbs((prev) => ({ ...prev, ...Object.fromEntries(batch.map((m) => [`${m.file.id}:${m.page}`, null])) })))
  }, [pages, files, thumbs, call])

  useEffect(() => {
    if (task.status === 'succeeded' && task.result) toast.success(t('organize.saved', { pages: task.result.pages, size: formatBytes(task.result.size) }))
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    try {
      await addPaths(await host.dialog.openFiles([{ name: 'PDF', extensions: ['pdf'] }]))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const click = (item: PageItem, event: React.MouseEvent) => {
    if (justDragged.current) {
      justDragged.current = false
      return
    }
    if (event.shiftKey && anchor) {
      const keys = pages.map((p) => p.key)
      const [from, to] = [keys.indexOf(anchor), keys.indexOf(item.key)].sort((a, b) => a - b)
      setSelected(new Set(keys.slice(from, to + 1)))
      return
    }
    const next = new Set(event.metaKey || event.ctrlKey ? selected : [])
    if (next.has(item.key) && (event.metaKey || event.ctrlKey)) next.delete(item.key)
    else next.add(item.key)
    setSelected(next)
    setAnchor(item.key)
  }
  const targets = (item?: PageItem) => (item && !selected.has(item.key) ? new Set([item.key]) : selected)
  const rotate = (delta: number, item?: PageItem) => {
    const keys = targets(item)
    setPages((prev) => prev.map((p) => (keys.has(p.key) ? { ...p, rotate: (p.rotate + delta + 360) % 360 } : p)))
  }
  const remove = (item?: PageItem) => {
    const keys = targets(item)
    setPages((prev) => prev.filter((p) => !keys.has(p.key)))
    setSelected(new Set())
  }
  const duplicate = () => {
    setPages((prev) => prev.flatMap((p) => (selected.has(p.key) ? [p, { ...p, key: key() }] : [p])))
  }
  const reset = () => {
    setPages(files.flatMap((f) => f.info.pages.map((p) => ({ key: key(), file: f.id, page: p.number, rotate: 0 }))))
    setSelected(new Set())
  }
  const removeFile = (id: number) => {
    setFiles((prev) => prev.filter((f) => f.id !== id))
    setPages((prev) => prev.filter((p) => p.file !== id))
  }
  // 把被拖动的页面（或选中的一组）移到目标页面之前或之后
  const move = (moving: Set<string>, to: DropTarget) => {
    if (to.key !== null && moving.has(to.key)) return
    setPages((prev) => {
      const moved = prev.filter((p) => moving.has(p.key))
      const rest = prev.filter((p) => !moving.has(p.key))
      let index = to.key === null ? rest.length : rest.findIndex((p) => p.key === to.key)
      if (index < 0) return prev
      if (to.key !== null && to.side === 'after') index += 1
      return [...rest.slice(0, index), ...moved, ...rest.slice(index)]
    })
  }

  // 窗口事件监听只注册一次，通过 ref 读取最新的选择与移动逻辑
  const selectedRef = useRef(selected)
  const moveRef = useRef(move)
  useEffect(() => {
    selectedRef.current = selected
    moveRef.current = move
  })

  const onPointerDown = (item: PageItem, event: React.PointerEvent) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button')) return
    pressed.current = { key: item.key, x: event.clientX, y: event.clientY }
  }

  // 根据指针位置找落点：指在某页的左半边放到它前面，右半边放到它后面；指在空白处放到最后
  const locate = (x: number, y: number, moving: Set<string>): DropTarget | null => {
    const element = document.elementFromPoint(x, y) as HTMLElement | null
    const card = element?.closest<HTMLElement>('[data-page-key]')
    if (card) {
      const key = card.dataset.pageKey!
      if (moving.has(key)) return null
      const rect = card.getBoundingClientRect()
      return { key, side: x < rect.left + rect.width / 2 ? 'before' : 'after' }
    }
    if (element && gridRef.current?.contains(element)) return { key: null, side: 'after' }
    return null
  }

  useEffect(() => {
    let frame = 0
    let pointer = { x: 0, y: 0 }
    let active: Set<string> | null = null
    const scroll = () => {
      const area = gridRef.current?.parentElement
      if (active && area) {
        const rect = area.getBoundingClientRect()
        const speed = pointer.y < rect.top + SCROLL_EDGE ? -12 : pointer.y > rect.bottom - SCROLL_EDGE ? 12 : 0
        if (speed) {
          area.scrollTop += speed
          setTarget(locate(pointer.x, pointer.y, active))
        }
      }
      frame = requestAnimationFrame(scroll)
    }
    const onMove = (event: PointerEvent) => {
      pointer = { x: event.clientX, y: event.clientY }
      const start = pressed.current
      if (!start) return
      if (!active) {
        if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < DRAG_THRESHOLD) return
        active = selectedRef.current.has(start.key) ? new Set(selectedRef.current) : new Set([start.key])
        setDragging(active)
        frame = requestAnimationFrame(scroll)
      }
      if (ghostRef.current) ghostRef.current.style.transform = `translate(${event.clientX + 12}px, ${event.clientY + 12}px)`
      setTarget(locate(event.clientX, event.clientY, active))
    }
    const finish = (commit: boolean) => {
      if (active && commit) {
        const to = locate(pointer.x, pointer.y, active)
        if (to) moveRef.current(active, to)
        justDragged.current = true
      }
      pressed.current = null
      active = null
      cancelAnimationFrame(frame)
      setDragging(null)
      setTarget(null)
    }
    const onUp = () => finish(true)
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && active) finish(false)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('keydown', onKey)
    return () => {
      cancelAnimationFrame(frame)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
      window.removeEventListener('keydown', onKey)
    }
  }, [])

  const exportPdf = async () => {
    if (pages.length === 0) return
    try {
      const base = files.length === 1 ? files[0].info.name.replace(/\.pdf$/i, '') + '-edited' : 'merged'
      const output = await host.dialog.saveFile(`${base}.pdf`, [{ name: 'PDF', extensions: ['pdf'] }])
      if (!output) return
      const order = files.map((f) => f.id)
      task.start('compose', {
        sources: files.map((f) => ({ path: f.info.path, password: f.password })),
        pages: pages.map((p) => ({ source: order.indexOf(p.file), page: p.page, rotate: p.rotate })),
        output,
        metadata: writeMetadata ? metadata : null,
        compress: true,
      })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const fileOf = useMemo(() => new Map(files.map((f) => [f.id, f])), [files])
  const ghostItem = dragging ? pages.find((p) => dragging.has(p.key)) : undefined
  const ghostThumb = ghostItem ? thumbs[`${ghostItem.file}:${ghostItem.page}`] : undefined

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_280px] gap-3">
      <Panel
        icon={<FileText />}
        title={t('organize.pages')}
        extra={pages.length > 0 && <Badge>{t('organize.count', { count: pages.length, selected: selected.size })}</Badge>}
        actions={
          <>
            <Tooltip content={t('organize.rotateLeft')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.rotateLeft')} disabled={selected.size === 0} onClick={() => rotate(-90)}>
                <RotateCcw />
              </Button>
            </Tooltip>
            <Tooltip content={t('organize.rotateRight')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.rotateRight')} disabled={selected.size === 0} onClick={() => rotate(90)}>
                <RotateCw />
              </Button>
            </Tooltip>
            <Tooltip content={t('organize.duplicate')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.duplicate')} disabled={selected.size === 0} onClick={duplicate}>
                <Copy />
              </Button>
            </Tooltip>
            <Tooltip content={t('organize.delete')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.delete')} disabled={selected.size === 0} onClick={() => remove()}>
                <Trash2 />
              </Button>
            </Tooltip>
            <Tooltip content={t('organize.reset')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.reset')} disabled={files.length === 0} onClick={reset}>
                <Undo2 />
              </Button>
            </Tooltip>
            <Button size="sm" onClick={pick}>
              <FilePlus2 />
              {t('organize.add')}
            </Button>
          </>
        }
        footer={<span className="text-[11px] text-fg-subtle">{t('organize.hint')}</span>}
        bodyClassName="overflow-auto p-3"
      >
        {pages.length === 0 ? (
          <button
            type="button"
            onClick={pick}
            className="flex h-full min-h-56 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed border-border text-center text-fg-muted outline-none hover:border-border-strong hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
          >
            <Upload className="size-6" />
            <span className="text-[13px] font-medium">{t('organize.drop')}</span>
            <span className="text-xs text-fg-subtle">{t('organize.dropHint')}</span>
          </button>
        ) : (
          <div
            ref={gridRef}
            className={cn('grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-3', dragging && 'cursor-grabbing')}
            onClick={(e) => e.target === e.currentTarget && setSelected(new Set())}
          >
            {pages.map((item, index) => {
              const file = fileOf.get(item.file)
              const thumb = thumbs[`${item.file}:${item.page}`]
              const active = selected.has(item.key)
              return (
                <div
                  key={item.key}
                  data-page-key={item.key}
                  onPointerDown={(e) => onPointerDown(item, e)}
                  onClick={(e) => click(item, e)}
                  className={cn(
                    'group relative flex cursor-pointer flex-col gap-1.5 rounded-card border p-2 transition-colors select-none',
                    active ? 'border-primary bg-primary-soft' : 'border-border bg-surface hover:border-border-strong',
                    dragging?.has(item.key) && 'opacity-40',
                    dragging && 'cursor-grabbing',
                  )}
                >
                  {/* 落点提示：页面之间的竖线 */}
                  {target?.key === item.key && (
                    <span
                      className={cn(
                        'pointer-events-none absolute top-0 bottom-0 z-10 w-1 rounded-full bg-primary shadow-[0_0_0_3px_var(--color-primary-soft)]',
                        target.side === 'before' ? '-left-2' : '-right-2',
                      )}
                    />
                  )}
                  <div className="flex aspect-[3/4] items-center justify-center overflow-hidden rounded-control bg-surface-2">
                    {thumb ? (
                      <img src={thumb} alt="" draggable={false} className="max-h-full max-w-full shadow-card transition-transform" style={{ transform: `rotate(${item.rotate}deg)` }} />
                    ) : thumb === null ? (
                      <FileText className="size-8 text-fg-subtle" />
                    ) : (
                      <span className="size-6 animate-pulse rounded-full bg-active" />
                    )}
                  </div>
                  <div className="flex items-center gap-1.5 text-[11px] text-fg-muted">
                    <span className={cn('size-2 shrink-0 rounded-full', file ? FILE_COLORS[file.color] : 'bg-border')} />
                    <span className="min-w-0 flex-1 truncate" title={file?.info.name}>{t('organize.pageOf', { page: item.page })}</span>
                    <span className="tabular-nums text-fg-subtle">{index + 1}</span>
                  </div>
                  <div className="absolute top-3 right-3 flex gap-0.5 opacity-0 group-hover:opacity-100">
                    <Button size="icon-sm" variant="outline" aria-label={t('organize.rotateRight')} onClick={(e) => { e.stopPropagation(); rotate(90, item) }}>
                      <RotateCw />
                    </Button>
                    <Button size="icon-sm" variant="outline" aria-label={t('organize.delete')} onClick={(e) => { e.stopPropagation(); remove(item) }}>
                      <Trash2 />
                    </Button>
                  </div>
                </div>
              )
            })}
            {dragging && target?.key === null && (
              <div className="flex aspect-[3/4] items-center justify-center rounded-card border-2 border-dashed border-primary bg-primary-soft text-xs text-primary-fg">
                {t('organize.dropEnd')}
              </div>
            )}
          </div>
        )}
        {dragging && (
          <div ref={ghostRef} className="pointer-events-none fixed top-0 left-0 z-50 w-20 opacity-90" style={{ transform: 'translate(-9999px, -9999px)' }}>
            <div className="relative rounded-control border border-primary bg-surface p-1 shadow-modal">
              {ghostThumb ? <img src={ghostThumb} alt="" className="w-full" /> : <FileText className="mx-auto size-8 text-fg-subtle" />}
              {dragging.size > 1 && (
                <span className="absolute -top-2 -right-2 grid h-5 min-w-5 place-items-center rounded-full bg-primary px-1 text-[11px] font-semibold text-fg-on-primary">
                  {dragging.size}
                </span>
              )}
            </div>
          </div>
        )}
      </Panel>

      <div className="flex min-h-0 flex-col gap-3">
        <Panel title={t('organize.files')} className="min-h-0 flex-1" bodyClassName="space-y-1 overflow-auto p-2">
          {files.length === 0 && <Empty title={t('organize.noFiles')} />}
          {files.map((file) => (
            <div key={file.id} className="group flex items-center gap-2 rounded-control px-2 py-1.5 hover:bg-hover">
              <span className={cn('size-2.5 shrink-0 rounded-full', FILE_COLORS[file.color])} />
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px] text-fg" title={file.info.path}>{fileName(file.info.path)}</p>
                <p className="text-[11px] text-fg-subtle tabular-nums">{t('organize.fileInfo', { pages: file.info.pages.length, size: formatBytes(file.info.size), version: file.info.version })}</p>
              </div>
              {file.info.encrypted && <Lock className="size-3.5 text-info" />}
              <Button size="icon-sm" variant="ghost" aria-label={t('organize.removeFile')} className="opacity-0 group-hover:opacity-100" onClick={() => removeFile(file.id)}>
                <X />
              </Button>
            </div>
          ))}
        </Panel>
        <Panel
          title={t('metadata.title')}
          actions={<Switch size="sm" checked={writeMetadata} onCheckedChange={setWriteMetadata} aria-label={t('metadata.write')} />}
          className="shrink-0"
          bodyClassName="space-y-2 p-3"
        >
          {(['title', 'author', 'subject', 'keywords'] as const).map((field) => (
            <Input
              key={field}
              size="sm"
              value={metadata?.[field] ?? ''}
              disabled={!writeMetadata}
              onChange={(e) => setMetadata({ ...(metadata ?? { title: '', author: '', subject: '', keywords: '', creator: '', producer: '' }), [field]: e.target.value })}
              placeholder={t(`metadata.${field}`)}
              aria-label={t(`metadata.${field}`)}
            />
          ))}
          <p className="text-[11px] text-fg-subtle">{t(writeMetadata ? 'metadata.writeHint' : 'metadata.keepHint')}</p>
        </Panel>
        {task.running && <Progress value={null} />}
        <Button variant="primary" loading={task.running} disabled={pages.length === 0} onClick={exportPdf}>
          <Save />
          {t('organize.export', { count: pages.length })}
        </Button>
        {task.status === 'succeeded' && task.result && (
          <Button size="sm" variant="ghost" onClick={() => host.revealPath(task.result!.output).catch((error) => toast.error(errorMessage(error)))}>
            {t('organize.show')}
          </Button>
        )}
      </div>
      {dialog}
    </div>
  )
}
