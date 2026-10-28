import { useCallback, useEffect, useRef, useState } from 'react'
import {
  Badge,
  Button,
  Empty,
  Input,
  NumberInput,
  Panel,
  Progress,
  SegmentedControl,
  Select,
  Spinner,
  Switch,
  Tooltip,
  cn,
  toast,
} from '@toolforge/ui'
import { callPlugin, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import {
  Brush,
  Copy,
  Eraser,
  FileDown,
  ImagePlus,
  Pause,
  Pencil,
  Play,
  Plus,
  Redo2,
  SkipBack,
  SkipForward,
  Trash2,
  Undo2,
  X,
} from 'lucide-react'
import { COLLECT_SLACK, createQueue, emptyHistory, record, redo, referenced, undo, type History } from './history'
import { Stage, type StrokeInput } from './Stage'
import {
  MAX_WORKS,
  deleteFrames,
  deleteWork,
  loadFrames,
  loadProject,
  loadWorks,
  metaOf,
  saveFrame,
  saveProject,
  saveWorks,
} from './store'
import { Timeline } from './Timeline'
import type { Edit, ExportOut, FrameOut, Limits, Op, Project, Tool, WorkMeta } from './types'

type Format = 'gif' | 'png'

/** 修改时附带的选项：新帧内容、是否记入撤销、操作后选中的帧 */
interface Change {
  content?: string
  undoable?: boolean
  select?: (project: Project) => number
}

export default function Flipbook() {
  const { t, manifest, errorMessage } = usePlugin()
  const pluginId = manifest.id
  const [limits, setLimits] = useState<Limits | null>(null)
  const [works, setWorks] = useState<WorkMeta[] | null>(null)
  const [project, setProject] = useState<Project | null>(null)
  const [contents, setContents] = useState<Record<string, string>>({})
  const [current, setCurrent] = useState(0)
  const [history, setHistory] = useState<History>(emptyHistory())
  const [tool, setTool] = useState<Tool>('brush')
  const [color, setColor] = useState('')
  const [size, setSize] = useState(6)
  const [onion, setOnion] = useState(true)
  const [playing, setPlaying] = useState(false)
  const [playIndex, setPlayIndex] = useState(0)
  const [pending, setPending] = useState<StrokeInput[]>([])
  const [busy, setBusy] = useState(0)
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')
  const [newSize, setNewSize] = useState('320x240')
  const [renaming, setRenaming] = useState<string | null>(null)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [format, setFormat] = useState<Format>('gif')
  const exporting = useTask<ExportOut>()
  const handledExport = useRef<string | null>(null)

  // 队列中的任务读取最新值，避免闭包读到旧作品
  const projectRef = useRef<Project | null>(null)
  const contentsRef = useRef<Record<string, string>>({})
  const historyRef = useRef<History>(emptyHistory())
  const currentRef = useRef(0)
  const worksRef = useRef<WorkMeta[]>([])
  const [queue] = useState(() => createQueue())

  const commitProject = (next: Project | null, nextContents?: Record<string, string>) => {
    projectRef.current = next
    setProject(next)
    if (nextContents) {
      contentsRef.current = nextContents
      setContents(nextContents)
    }
  }
  const commitHistory = (next: History) => {
    historyRef.current = next
    setHistory(next)
  }
  const select = (index: number) => {
    currentRef.current = index
    setCurrent(index)
  }
  const commitWorks = async (next: WorkMeta[]) => {
    await saveWorks(pluginId, next)
    worksRef.current = next
    setWorks(next)
  }

  /** 串行执行一个修改；失败时提示，作品保持修改前的样子 */
  const run = useCallback(
    (task: () => Promise<void>) => {
      setBusy((n) => n + 1)
      return queue.push(async () => {
        try {
          await task()
        } catch (error) {
          toast.error(errorMessage(error))
        } finally {
          setBusy((n) => n - 1)
        }
      })
    },
    [queue, errorMessage],
  )

  /** 只停止播放；切换工具、修改、切换作品、导出与卸载都会调用，不会留下后台播放 */
  const stop = () => setPlaying(false)

  /**
   * 应用一次帧操作：Rust 校验并给出新作品 → 写入新帧内容 → 写入作品结构 → 更新界面。
   * 任何一步失败都不更新界面；作品结构写入失败时删掉刚写的帧，原作品不受影响。
   */
  const applyEdit = async (op: Op, change: Change = {}) => {
    const before = projectRef.current
    if (!before) return
    const sizes = Object.fromEntries(before.frames.map((id) => [id, (contentsRef.current[id] ?? '').length]))
    const edit = await callPlugin<Edit>(pluginId, 'edit', { project: before, op, sizes })
    let nextContents: Record<string, string> | undefined
    if (edit.created) {
      const { id, source } = edit.created
      const content = change.content ?? (source ? (contentsRef.current[source] ?? '') : '')
      await saveFrame(pluginId, before.id, id, content)
      nextContents = { ...contentsRef.current, [id]: content }
    }
    try {
      await saveProject(pluginId, edit.project)
    } catch (error) {
      if (edit.created) await deleteFrames(pluginId, before.id, [edit.created.id]).catch(() => undefined)
      throw error
    }
    if (change.undoable) commitHistory(record(historyRef.current, before.frames))
    commitProject(edit.project, nextContents)
    await collectGarbage()
    const index = change.select ? change.select(edit.project) : Math.min(currentRef.current, edit.project.frames.length - 1)
    select(Math.max(0, Math.min(index, edit.project.frames.length - 1)))
    // 列表摘要写入失败不影响作品本身
    const meta = metaOf(edit.project)
    await commitWorks(worksRef.current.map((w) => (w.id === meta.id ? meta : w))).catch(() => undefined)
  }

  /**
   * 会话中的存储清理：撤销栈超出深度后，最早的旧帧不再被引用；
   * 多出的帧达到阈值时删掉不在当前顺序、也不在撤销历史里的帧。失败不影响作品，下次打开时还会再清理。
   */
  const collectGarbage = async () => {
    const p = projectRef.current
    if (!p || p.pool.length - p.frames.length < COLLECT_SLACK) return
    try {
      const out = await callPlugin<{ project: Project; remove: string[] }>(pluginId, 'collect', {
        project: p,
        keep: referenced(historyRef.current),
      })
      if (out.remove.length === 0) return
      await saveProject(pluginId, out.project)
      commitProject(out.project)
      await deleteFrames(pluginId, p.id, out.remove)
    } catch {
      // 清理是尽力而为
    }
  }

  const edit = (op: Op, change: Change = {}) => {
    stop()
    return run(() => applyEdit(op, change))
  }

  /** 打开作品：读取结构与帧，清理不再被引用的旧帧（新打开时没有撤销历史） */
  const openWork = (id: string) => {
    stop()
    return run(async () => {
      const saved = await loadProject(pluginId, id)
      if (!saved) {
        toast.error(t('errors.missingWork'))
        return
      }
      const out = await callPlugin<{ project: Project; remove: string[] }>(pluginId, 'collect', { project: saved, keep: [] })
      if (out.remove.length > 0) {
        await saveProject(pluginId, out.project)
        await deleteFrames(pluginId, id, out.remove).catch(() => undefined)
      }
      const loaded = await loadFrames(pluginId, id, out.project.frames)
      commitHistory(emptyHistory())
      commitProject(out.project, loaded)
      select(0)
      setPending([])
    })
  }

  // 首次加载：限制、调色板与作品列表，自动打开最近编辑的作品
  useEffect(() => {
    let alive = true
    Promise.all([callPlugin<Limits>(pluginId, 'limits', {}), loadWorks(pluginId)])
      .then(([l, list]) => {
        if (!alive) return
        setLimits(l)
        setColor(l.palette[0])
        const sorted = [...list].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
        worksRef.current = sorted
        setWorks(sorted)
        if (sorted[0]) void openWork(sorted[0].id)
        else setCreating(true)
      })
      .catch((error) => toast.error(errorMessage(error)))
    return () => {
      alive = false
    }
    // 只在打开工具时执行一次
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pluginId])

  const createWork = () => {
    const [width, height] = newSize.split('x').map(Number)
    const name = newName.trim() || t('work.untitled')
    if (worksRef.current.length >= MAX_WORKS) {
      toast.error(t('errors.tooManyWorks', { max: MAX_WORKS }))
      return
    }
    stop()
    return run(async () => {
      const id = `w${Date.now().toString(36)}`
      const created = await callPlugin<Project>(pluginId, 'create', { id, name, width, height })
      await saveProject(pluginId, created)
      await commitWorks([metaOf(created), ...worksRef.current])
      commitHistory(emptyHistory())
      commitProject(created, {})
      select(0)
      setCreating(false)
      setNewName('')
    })
  }

  const removeWork = () => {
    const target = projectRef.current
    if (!target) return
    if (!confirmDelete) {
      setConfirmDelete(true)
      return
    }
    setConfirmDelete(false)
    stop()
    void run(async () => {
      await deleteWork(pluginId, target)
      const rest = worksRef.current.filter((w) => w.id !== target.id)
      await commitWorks(rest)
      commitProject(null, {})
      commitHistory(emptyHistory())
      if (rest[0]) await openWorkNow(rest[0].id)
      else setCreating(true)
    })
  }

  /** 在已有队列任务里直接打开作品（不能再排队，否则会等待自己） */
  const openWorkNow = async (id: string) => {
    const saved = await loadProject(pluginId, id)
    if (!saved) return
    const loaded = await loadFrames(pluginId, id, saved.frames)
    commitProject(saved, loaded)
    select(0)
  }

  const onStroke = (stroke: StrokeInput) => {
    stop()
    setPending((list) => [...list, stroke])
    void run(async () => {
      try {
        const p = projectRef.current
        if (!p) return
        const index = currentRef.current
        const out = await callPlugin<FrameOut>(pluginId, 'stroke', {
          width: p.width,
          height: p.height,
          frame: contentsRef.current[p.frames[index]] ?? '',
          stroke,
        })
        await applyEdit({ kind: 'replace', index, size: out.size }, { content: out.frame, undoable: true, select: () => index })
      } finally {
        setPending((list) => list.filter((s) => s !== stroke))
      }
    })
  }

  const importImage = async () => {
    stop()
    let path: string | null = null
    try {
      path = await host.dialog.openFile([{ name: t('import.images'), extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }])
    } catch (error) {
      toast.error(errorMessage(error))
    }
    if (!path) return
    const picked = path
    void run(async () => {
      const p = projectRef.current
      if (!p) return
      const index = currentRef.current
      const out = await callPlugin<FrameOut>(pluginId, 'import', {
        width: p.width,
        height: p.height,
        frame: contentsRef.current[p.frames[index]] ?? '',
        path: picked,
      })
      await applyEdit({ kind: 'replace', index, size: out.size }, { content: out.frame, undoable: true, select: () => index })
    })
  }

  const undoStep = () => {
    stop()
    void run(async () => {
      const p = projectRef.current
      const step = p && undo(historyRef.current, p.frames)
      if (!step) return
      await applyEdit({ kind: 'restore', frames: step.frames })
      commitHistory(step.history)
    })
  }
  const redoStep = () => {
    stop()
    void run(async () => {
      const p = projectRef.current
      const step = p && redo(historyRef.current, p.frames)
      if (!step) return
      await applyEdit({ kind: 'restore', frames: step.frames })
      commitHistory(step.history)
    })
  }

  // 快捷键：⌘/Ctrl+Z 撤销，⇧⌘/Ctrl+Shift+Z 或 Ctrl+Y 重做（输入框内不拦截）
  const shortcutRef = useRef({ undoStep, redoStep })
  useEffect(() => {
    shortcutRef.current = { undoStep, redoStep }
  })
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) return
      const mod = event.metaKey || event.ctrlKey
      if (!mod) return
      const key = event.key.toLowerCase()
      if (key === 'z' && !event.shiftKey) {
        event.preventDefault()
        shortcutRef.current.undoStep()
      } else if ((key === 'z' && event.shiftKey) || key === 'y') {
        event.preventDefault()
        shortcutRef.current.redoStep()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  // 播放：按真实流逝时间推进，停留时长与导出一致；停止或卸载时取消动画帧
  useEffect(() => {
    if (!playing || !project) return
    let raf = 0
    let last = performance.now()
    let acc = 0
    let index = currentRef.current
    const count = project.frames.length
    const delay = project.delayMs
    const tick = (now: number) => {
      acc += now - last
      last = now
      if (acc >= delay) {
        index = (index + Math.floor(acc / delay)) % count
        acc %= delay
        setPlayIndex(index)
      }
      raf = requestAnimationFrame(tick)
    }
    raf = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(raf)
  }, [playing, project])

  const togglePlay = () => {
    if (playing) {
      // 暂停时停在正在播放的那一帧
      select(playIndex)
      setPlaying(false)
    } else {
      setPlayIndex(currentRef.current)
      setPlaying(true)
    }
  }

  const step = (delta: number) => {
    if (!project) return
    stop()
    select((current + delta + project.frames.length) % project.frames.length)
  }

  const startExport = async () => {
    const p = projectRef.current
    if (!p) return
    stop()
    try {
      const path =
        format === 'gif'
          ? await host.dialog.saveFile(`${p.name}.gif`, [{ name: 'GIF', extensions: ['gif'] }])
          : await host.dialog.openDirectory()
      if (!path) return
      // 等排队中的修改写完，导出的就是界面上看到的作品
      await queue.push(async () => undefined)
      const latest = projectRef.current
      if (!latest) return
      exporting.reset()
      await exporting.start('export', {
        format,
        width: latest.width,
        height: latest.height,
        delayMs: latest.delayMs,
        background: latest.background,
        frames: latest.frames.map((id) => contentsRef.current[id] ?? ''),
        path,
        name: latest.name,
      })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  useEffect(() => {
    const id = exporting.taskId
    if (!id || exporting.running || exporting.status === 'idle' || handledExport.current === id) return
    handledExport.current = id
    if (exporting.status === 'succeeded' && exporting.result) {
      toast.success(t('export.done', { count: exporting.result.files }))
    } else if (exporting.status === 'cancelled') {
      toast.info(t('export.cancelled'))
    } else if (exporting.error) {
      toast.error(errorMessage(exporting.error))
    }
  }, [exporting.taskId, exporting.running, exporting.status, exporting.result, exporting.error, t, errorMessage])

  // 离开工具时停止播放（动画帧在 effect 清理中取消）；导出任务由宿主任务中心管理
  useEffect(() => () => setPlaying(false), [])

  if (!limits || works === null) {
    return (
      <div className="flex h-full items-center justify-center p-3">
        <Spinner />
      </div>
    )
  }

  const sizeOptions = limits.sizes.map((s) => ({ value: `${s.width}x${s.height}`, label: `${s.width} × ${s.height}` }))
  const locked = playing || exporting.running
  const shown = project ? (playing ? project.frames[playIndex % project.frames.length] : project.frames[current]) : undefined
  const fps = project ? 1000 / project.delayMs : 0
  const loopSeconds = project ? (project.delayMs * project.frames.length) / 1000 : 0

  return (
    <div className="space-y-3 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={project?.id ?? null}
          onValueChange={(id) => void openWork(id)}
          options={works.map((w) => ({ value: w.id, label: `${w.name} · ${w.frames}` }))}
          placeholder={t('work.choose')}
          size="sm"
          className="w-56"
          disabled={locked}
        />
        <Button size="sm" variant="secondary" onClick={() => setCreating(true)} disabled={locked}>
          <Plus />
          {t('work.new')}
        </Button>
        {project && renaming === null && (
          <Tooltip content={t('work.rename')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('work.rename')} onClick={() => setRenaming(project.name)} disabled={locked}>
              <Pencil />
            </Button>
          </Tooltip>
        )}
        {project && renaming !== null && (
          <form
            className="flex items-center gap-1"
            onSubmit={(event) => {
              event.preventDefault()
              const name = renaming
              setRenaming(null)
              void edit({ kind: 'rename', name })
            }}
          >
            <Input size="sm" value={renaming} onChange={(e) => setRenaming(e.target.value)} maxLength={60} autoFocus />
            <Button size="sm" type="submit">
              {t('actions.save')}
            </Button>
            <Button size="icon-sm" variant="ghost" aria-label={t('actions.cancel')} onClick={() => setRenaming(null)}>
              <X />
            </Button>
          </form>
        )}
        {project && (
          <Tooltip content={confirmDelete ? t('work.deleteConfirm') : t('work.delete')}>
            <Button
              size="icon-sm"
              variant={confirmDelete ? 'danger' : 'ghost'}
              aria-label={t('work.delete')}
              onClick={removeWork}
              onBlur={() => setConfirmDelete(false)}
              disabled={locked}
            >
              <Trash2 />
            </Button>
          </Tooltip>
        )}
        <div className="ml-auto flex items-center gap-2 text-[12px] text-fg-muted">
          {busy > 0 && <Spinner />}
          {project && <Badge>{`${project.width} × ${project.height}`}</Badge>}
          {project && <Badge>{t('frames.count', { count: project.frames.length, max: limits.maxFrames })}</Badge>}
        </div>
      </div>

      {creating && (
        <Panel title={t('work.newTitle')} bodyClassName="flex flex-wrap items-end gap-2 p-3">
          <label className="space-y-1 text-[12px] text-fg-muted">
            <span>{t('work.name')}</span>
            <Input size="sm" value={newName} onChange={(e) => setNewName(e.target.value)} placeholder={t('work.untitled')} maxLength={60} />
          </label>
          <label className="space-y-1 text-[12px] text-fg-muted">
            <span>{t('work.size')}</span>
            <Select value={newSize} onValueChange={setNewSize} options={sizeOptions} size="sm" className="w-40" />
          </label>
          <Button size="sm" onClick={() => void createWork()}>
            {t('work.create')}
          </Button>
          {project && (
            <Button size="sm" variant="ghost" onClick={() => setCreating(false)}>
              {t('actions.cancel')}
            </Button>
          )}
          <p className="w-full text-[12px] text-fg-muted">{t('work.sizeHint')}</p>
        </Panel>
      )}

      {!project && !creating && <Empty title={t('work.empty')} />}

      {project && (
        <>
          <Panel bodyClassName="space-y-3 p-3">
            <div className="flex flex-wrap items-center gap-2">
              <SegmentedControl<Tool>
                size="sm"
                value={tool}
                onValueChange={(value) => {
                  stop()
                  setTool(value)
                }}
                options={[
                  { value: 'brush', label: <span className="flex items-center gap-1"><Brush className="size-3.5" />{t('tool.brush')}</span> },
                  { value: 'eraser', label: <span className="flex items-center gap-1"><Eraser className="size-3.5" />{t('tool.eraser')}</span> },
                ]}
              />
              <div className="flex items-center gap-1" role="radiogroup" aria-label={t('tool.color')}>
                {limits.palette.map((c) => (
                  <button
                    key={c}
                    type="button"
                    role="radio"
                    aria-checked={color === c}
                    aria-label={c}
                    onClick={() => {
                      stop()
                      setColor(c)
                      setTool('brush')
                    }}
                    className={cn(
                      'size-5 rounded-full border border-border-strong outline-none focus-visible:ring-2 focus-visible:ring-ring',
                      color === c && tool === 'brush' && 'ring-2 ring-primary ring-offset-1 ring-offset-surface',
                    )}
                    style={{ backgroundColor: c }}
                  />
                ))}
              </div>
              <label className="flex items-center gap-1 text-[12px] text-fg-muted">
                {t('tool.size')}
                <NumberInput
                  size="sm"
                  value={size}
                  min={limits.minBrush}
                  max={limits.maxBrush}
                  step={1}
                  onValueChange={(value) => {
                    stop()
                    setSize(value)
                  }}
                  className="w-24"
                />
              </label>
              <label className="flex items-center gap-2 text-[12px] text-fg-muted">
                <Switch size="sm" checked={onion} onCheckedChange={setOnion} />
                {t('tool.onion')}
              </label>
              <div className="ml-auto flex items-center gap-1">
                <Tooltip content={t('history.undo')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('history.undo')} onClick={undoStep} disabled={locked || history.past.length === 0}>
                    <Undo2 />
                  </Button>
                </Tooltip>
                <Tooltip content={t('history.redo')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('history.redo')} onClick={redoStep} disabled={locked || history.future.length === 0}>
                    <Redo2 />
                  </Button>
                </Tooltip>
                <Tooltip content={t('frames.import')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('frames.import')} onClick={() => void importImage()} disabled={locked}>
                    <ImagePlus />
                  </Button>
                </Tooltip>
              </div>
            </div>

            <Stage
              width={project.width}
              height={project.height}
              content={shown ? contents[shown] : undefined}
              onion={onion && !playing && current > 0 ? contents[project.frames[current - 1]] : undefined}
              tool={tool}
              color={color}
              size={size}
              disabled={locked}
              pending={pending}
              onStroke={onStroke}
              label={t('stage.label', { index: current + 1 })}
            />

            <div className="flex flex-wrap items-center gap-2 text-[12px]">
              <Tooltip content={t('play.prev')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('play.prev')} onClick={() => step(-1)} disabled={exporting.running}>
                  <SkipBack />
                </Button>
              </Tooltip>
              <Tooltip content={playing ? t('play.pause') : t('play.play')}>
                <Button size="icon-sm" variant="primary" aria-label={playing ? t('play.pause') : t('play.play')} onClick={togglePlay} disabled={exporting.running}>
                  {playing ? <Pause /> : <Play />}
                </Button>
              </Tooltip>
              <Tooltip content={t('play.next')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('play.next')} onClick={() => step(1)} disabled={exporting.running}>
                  <SkipForward />
                </Button>
              </Tooltip>
              <span className="text-fg-muted">
                {t('play.position', { index: (playing ? playIndex : current) + 1, count: project.frames.length })}
              </span>
              <label className="ml-auto flex items-center gap-1 text-fg-muted">
                {t('play.delay')}
                <NumberInput
                  size="sm"
                  value={project.delayMs}
                  min={limits.minDelay}
                  max={limits.maxDelay}
                  step={limits.delayStep}
                  onValueChange={(ms) => void edit({ kind: 'delay', ms })}
                  className="w-28"
                  disabled={exporting.running}
                />
              </label>
              <span className="text-fg-muted">{t('play.speed', { fps: fps.toFixed(1), seconds: loopSeconds.toFixed(2) })}</span>
            </div>
          </Panel>

          <Panel
            title={t('frames.title')}
            bodyClassName="space-y-2 p-3"
            extra={
              <div className="flex items-center gap-1">
                <Tooltip content={t('frames.add')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('frames.add')} onClick={() => void edit({ kind: 'add', index: current }, { undoable: true, select: () => current + 1 })} disabled={locked}>
                    <Plus />
                  </Button>
                </Tooltip>
                <Tooltip content={t('frames.duplicate')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('frames.duplicate')} onClick={() => void edit({ kind: 'duplicate', index: current }, { undoable: true, select: () => current + 1 })} disabled={locked}>
                    <Copy />
                  </Button>
                </Tooltip>
                <Tooltip content={t('frames.delete')}>
                  <Button size="icon-sm" variant="ghost" aria-label={t('frames.delete')} onClick={() => void edit({ kind: 'delete', index: current }, { undoable: true })} disabled={locked || project.frames.length <= 1}>
                    <Trash2 />
                  </Button>
                </Tooltip>
              </div>
            }
          >
            <Timeline
              frames={project.frames}
              contents={contents}
              width={project.width}
              height={project.height}
              current={playing ? playIndex % project.frames.length : current}
              disabled={locked}
              onSelect={(index) => {
                stop()
                select(index)
              }}
              onMove={(from, to) => void edit({ kind: 'move', from, to }, { undoable: true, select: () => to })}
              frameLabel={(index) => t('frames.label', { index: index + 1 })}
            />
            <p className="text-[12px] text-fg-muted">{t('frames.hint')}</p>
          </Panel>

          <Panel title={t('export.title')} bodyClassName="space-y-2 p-3">
            <div className="flex flex-wrap items-center gap-2">
              <SegmentedControl<Format>
                size="sm"
                value={format}
                onValueChange={setFormat}
                options={[
                  { value: 'gif', label: t('export.gif') },
                  { value: 'png', label: t('export.png') },
                ]}
              />
              {format === 'gif' && (
                <div className="flex items-center gap-1" role="radiogroup" aria-label={t('export.background')}>
                  <span className="text-[12px] text-fg-muted">{t('export.background')}</span>
                  {limits.backgrounds.map((c) => (
                    <button
                      key={c}
                      type="button"
                      role="radio"
                      aria-checked={project.background === c}
                      aria-label={c}
                      disabled={exporting.running}
                      onClick={() => void edit({ kind: 'background', color: c })}
                      className={cn(
                        'size-5 rounded border border-border-strong outline-none focus-visible:ring-2 focus-visible:ring-ring',
                        project.background === c && 'ring-2 ring-primary ring-offset-1 ring-offset-surface',
                      )}
                      style={{ backgroundColor: c }}
                    />
                  ))}
                </div>
              )}
              <div className="ml-auto flex items-center gap-2">
                {exporting.running ? (
                  <Button size="sm" variant="secondary" onClick={() => void exporting.cancel()}>
                    <X />
                    {t('export.cancel')}
                  </Button>
                ) : (
                  <Button size="sm" onClick={() => void startExport()} disabled={busy > 0}>
                    <FileDown />
                    {t('export.start')}
                  </Button>
                )}
              </div>
            </div>
            {exporting.running && (
              <Progress value={exporting.progress ? (exporting.progress.done / Math.max(1, exporting.progress.total)) * 100 : 0} />
            )}
            <p className="text-[12px] text-fg-muted">{format === 'gif' ? t('export.gifHint') : t('export.pngHint')}</p>
          </Panel>
        </>
      )}
    </div>
  )
}
