import { useMemo, useState } from 'react'
import { Badge, Button, Checkbox, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Eraser, Grid3x3, Lightbulb, Pause, Play, RotateCcw } from 'lucide-react'

type Size = 4 | 9
interface Cell {
  value: number
  notes: number[]
}
interface Saved {
  size: Size
  puzzle: number[]
  solution: number[]
  cells: Cell[]
}

interface Conflict {
  index: number
  kind: string
}

interface AnalyzeOut {
  status: string
  conflicts: Conflict[]
  blanks: number
}

interface HintOut {
  index: number
  value: number
  reason: string
}

const emptyCells = (n: number): Cell[] => Array.from({ length: n }, () => ({ value: 0, notes: [] }))

export default function Sudoku() {
  const { t, errorMessage, call } = usePlugin()
  const [selected, setSelected] = useState<number | null>(null)
  const [notesMode, setNotesMode] = useState(false)
  const [showConflicts, setShowConflicts] = useState(true)
  const [paused, setPaused] = useState(false)
  const [history, setHistory] = useState<Cell[][]>([])
  const [future, setFuture] = useState<Cell[][]>([])
  const [hint, setHint] = useState<HintOut | null>(null)
  const [busy, setBusy] = useState(false)
  const [saved, saveProgress] = usePluginState<Saved>('game', { size: 9, puzzle: [], solution: [], cells: emptyCells(81) })
  // 本局覆盖状态；未开新局时优先显示已存对局
  const [session, setSession] = useState<Saved | null>(null)
  const active: Saved = session ?? (saved.puzzle.length > 0 ? saved : { size: 9, puzzle: [], solution: [], cells: emptyCells(81) })
  const size = active.size
  const puzzle = active.puzzle
  const solution = active.solution
  const cells = active.cells

  const analyze = useDebouncedCall<AnalyzeOut>(puzzle.length > 0 ? 'analyze' : null, { size, grid: cells.map((c) => c.value) }, [size, cells])
  const conflicts = useMemo(() => {
    if (!showConflicts) return new Set<number>()
    if (analyze.result?.status === 'conflicts') return new Set(analyze.result.conflicts.map((c) => c.index))
    return new Set<number>()
  }, [analyze.result, showConflicts])

  const newGame = async (nextSize: Size) => {
    setBusy(true)
    setHint(null)
    try {
      const holes = nextSize === 4 ? 8 : 45
      const seed = Date.now() % 1_000_000_007
      const out = await call<{ puzzle: number[]; solution: number[] }>('generate', { size: nextSize, holes, seed })
      const fresh = emptyCells(nextSize * nextSize)
      out.puzzle.forEach((value, index) => {
        if (value > 0) fresh[index] = { value, notes: [] }
      })
      setSession({ size: nextSize, puzzle: out.puzzle, solution: out.solution, cells: fresh })
      setHistory([])
      setFuture([])
      setSelected(null)
      setPaused(false)
      saveProgress({ size: nextSize, puzzle: out.puzzle, solution: out.solution, cells: fresh })
    } finally {
      setBusy(false)
    }
  }

  const commit = (next: Cell[]) => {
    setHistory((prev) => [...prev.slice(-200), cells])
    setFuture([])
    setSession({ size, puzzle, solution, cells: next })
    if (puzzle.length > 0) saveProgress({ size, puzzle, solution, cells: next })
  }

  const setCell = (index: number, value: number) => {
    if (index === null || index < 0 || puzzle[index] > 0) return
    const next = cells.map((cell, i) => {
      if (i !== index) return cell
      if (notesMode && value > 0) {
        const notes = cell.notes.includes(value) ? cell.notes.filter((n) => n !== value) : [...cell.notes, value].sort()
        return { value: 0, notes }
      }
      return { value: value === cell.value ? 0 : value, notes: [] }
    })
    commit(next)
  }

  const erase = () => {
    if (selected === null) return
    commit(cells.map((cell, i) => (i === selected ? { value: 0, notes: [] } : cell)))
  }

  const undo = () => {
    setHistory((prev) => {
      if (prev.length === 0) return prev
      setFuture((f) => [cells, ...f])
      const last = prev[prev.length - 1]
      setSession({ size, puzzle, solution, cells: last })
      return prev.slice(0, -1)
    })
  }

  const redo = () => {
    setFuture((prev) => {
      if (prev.length === 0) return prev
      setHistory((h) => [...h, cells])
      setSession({ size, puzzle, solution, cells: prev[0] })
      return prev.slice(1)
    })
  }

  const askHint = async () => {
    setBusy(true)
    try {
      const out = await call<HintOut>('hint', { size, grid: cells.map((c) => c.value), solution })
      setHint(out)
      setSelected(out.index)
    } catch {
      setHint(null)
    } finally {
      setBusy(false)
    }
  }

  const blanks = cells.filter((c) => c.value === 0).length
  const bs = size === 4 ? 2 : 3

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <SegmentedControl
          size="sm"
          value={String(size)}
          onValueChange={(value) => void newGame(Number(value) as Size)}
          aria-label={t('ui.size')}
          options={[
            { value: '4', label: '4×4' },
            { value: '9', label: '9×9' },
          ]}
        />
        <Button size="sm" variant="secondary" onClick={() => void newGame(size)} disabled={busy}>
          <RotateCcw className="size-3.5" />
          {t('ui.new')}
        </Button>
        <Button size="sm" variant="secondary" onClick={() => setPaused((prev) => !prev)}>
          {paused ? <Play className="size-3.5" /> : <Pause className="size-3.5" />}
          {paused ? t('brew.play') : t('brew.pause')}
        </Button>
        <Checkbox checked={showConflicts} onCheckedChange={(checked) => setShowConflicts(checked === true)} >{t('ui.conflictsOn')}</Checkbox>
        <Badge variant="neutral">{t('ui.progress')}: {size * size - blanks}/{size * size}</Badge>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(240px,320px)] gap-3">
        <Panel icon={<Grid3x3 />} title={t('ui.board')} className="min-h-0" bodyClassName="flex items-center justify-center p-3" extra={busy && <Spinner className="size-3.5" />}>
          {paused ? (
            <p className="text-sm text-fg-subtle">{t('ui.paused')}</p>
          ) : (
            <div
              className="grid select-none gap-0.5"
              style={{ gridTemplateColumns: `repeat(${size}, minmax(0, 1fr))`, width: 'min(100%, 60vh)' }}
              role="grid"
              aria-label={t('ui.board')}
            >
              {cells.map((cell, index) => {
                const given = puzzle[index] > 0
                const bold = size === 9 && ((index % 9) % 3 === 2 || Math.floor(index / 9) % 3 === 2)
                return (
                  <button
                    key={index}
                    type="button"
                    onClick={() => setSelected(index)}
                    className={`flex aspect-square items-center justify-center border text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
                      selected === index ? 'border-primary bg-primary-soft' : conflicts.has(index) ? 'border-danger bg-danger-soft' : 'border-border bg-surface'
                    } ${bold ? 'border-r-2 border-r-border-strong' : ''} ${given ? 'font-semibold text-fg' : 'text-primary-fg'}`}
                  >
                    {cell.value > 0 ? (
                      cell.value
                    ) : cell.notes.length > 0 ? (
                      <span className="grid w-full grid-cols-2 gap-px p-0.5 text-[8px] leading-none text-fg-subtle" style={{ gridTemplateColumns: `repeat(${bs}, 1fr)` }}>
                        {Array.from({ length: size }, (_, n) => (
                          <span key={n}>{cell.notes.includes(n + 1) ? n + 1 : ''}</span>
                        ))}
                      </span>
                    ) : (
                      ''
                    )}
                  </button>
                )
              })}
            </div>
          )}
        </Panel>
        <div className="flex min-h-0 flex-col gap-3">
          <Panel title={t('ui.numbers')} bodyClassName="p-3">
            <div className="grid grid-cols-5 gap-1.5">
              {Array.from({ length: size }, (_, n) => (
                <Button key={n} size="sm" variant="secondary" onClick={() => setCell(selected ?? -1, n + 1)} disabled={selected === null}>
                  {n + 1}
                </Button>
              ))}
              <Button size="sm" variant="ghost" onClick={erase} disabled={selected === null} aria-label={t('ui.erase')}>
                <Eraser className="size-3.5" />
              </Button>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-3">
              <Checkbox checked={notesMode} onCheckedChange={(checked) => setNotesMode(checked === true)} >{t('ui.notes')}</Checkbox>
              <div className="flex gap-1.5">
                <Button size="sm" variant="ghost" onClick={undo} disabled={history.length === 0} aria-label={t('ui.undo')}>
                  {t('ui.undo')}
                </Button>
                <Button size="sm" variant="ghost" onClick={redo} disabled={future.length === 0} aria-label={t('ui.redo')}>
                  {t('ui.redo')}
                </Button>
              </div>
            </div>
          </Panel>
          <Panel title={t('ui.assist')} bodyClassName="space-y-2 p-3">
            <Button size="sm" variant="secondary" onClick={() => void askHint()} disabled={busy || puzzle.length === 0}>
              <Lightbulb className="size-3.5" />
              {t('ui.hint')}
            </Button>
            {hint && (
              <div className="rounded-control border border-border p-2 text-xs">
                <p className="font-medium text-fg">
                  {t('cell', { row: Math.floor(hint.index / size) + 1, col: (hint.index % size) + 1 })} = {hint.value}
                </p>
                <p className="mt-0.5 text-fg-muted">{t(`hints.${hint.reason}`)}</p>
              </div>
            )}
            {analyze.result?.status === 'conflicts' && showConflicts && (
              <p className="text-xs text-danger">
                {t('status.conflicts')}: {analyze.result.conflicts.length}
              </p>
            )}
            {analyze.result?.status === 'solved' && <Badge variant="success">{t('status.solved')}</Badge>}
            {analyze.error && <p className="text-xs text-danger">{errorMessage(analyze.error)}</p>}
            {saved && saved.puzzle.length > 0 && <p className="text-[11px] text-fg-subtle">{t('ui.saved')}</p>}
          </Panel>
        </div>
      </div>
    </div>
  )
}
