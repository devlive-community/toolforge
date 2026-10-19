import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Input, Panel, Spinner, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import {
  ArrowLeftRight,
  Download,
  Eraser,
  Lightbulb,
  Pause,
  RotateCcw,
  RotateCw,
  Sparkles,
} from 'lucide-react'
import type { CheckOut, Clues, HintOut, NonogramState, PaperOut, Puzzle } from './types'
import { CELL_BLACK, CELL_UNKNOWN, CELL_WHITE, initialNonogramState } from './types'

const SIZES = [5, 10, 15]

function localToday(): string {
  const now = new Date()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${now.getFullYear()}-${month}-${day}`
}

export default function Nonogram() {
  const { t, errorMessage, manifest } = usePlugin()
  const [state, setState] = usePluginState<NonogramState>('game', initialNonogramState())
  const [pending, setPending] = useState(false)
  const [selected, setSelected] = useState<number | null>(null)
  const [hintText, setHintText] = useState<string | null>(null)
  const [showSolution, setShowSolution] = useState(false)
  const [paperTitle, setPaperTitle] = useState('')
  // 拖动状态：按下时的目标动作（paint=涂黑 clear=标空 erase=取消黑）
  const drag = useRef<{ active: boolean; action: 'paint' | 'clear' | 'erase'; touched: Set<number> }>({
    active: false,
    action: 'paint',
    touched: new Set(),
  })

  const size = state.clues?.size ?? state.size
  const board = state.board

  // ---- 开新局 ----
  const newGame = async (size: number) => {
    setPending(true)
    try {
      const puzzle = await callPlugin<Puzzle>(manifest.id, 'generate', { size })
      setState({
        ...initialNonogramState(),
        size,
        clues: puzzle.clues,
        solution: puzzle.solution,
        board: Array(puzzle.solution.length).fill(CELL_UNKNOWN),
      })
      setHintText(null)
      setShowSolution(false)
      setSelected(null)
    } catch (reason) {
      toast.error(errorMessage(reason))
    } finally {
      setPending(false)
    }
  }

  // 首次进入自动开一局 5×5（异步触发，避免 effect 内同步 setState）
  useEffect(() => {
    if (!state.clues && !pending) {
      const timer = setTimeout(() => void newGame(5), 0)
      return () => clearTimeout(timer)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // ---- 历史管理：一次拖动 = 一步 ----
  const pushHistory = () => {
    const snapshot = [...(board ?? [])]
    setState({
      ...state,
      history: [...state.history.slice(-999), snapshot],
      redo: [],
    })
  }

  const undo = () => {
    if (state.history.length === 0 || !board) return
    const previous = state.history[state.history.length - 1]
    setState({
      ...state,
      board: previous,
      history: state.history.slice(0, -1),
      redo: [...state.redo, [...board]],
    })
  }

  const redoStep = () => {
    if (state.redo.length === 0 || !board) return
    const next = state.redo[state.redo.length - 1]
    setState({
      ...state,
      board: next,
      history: [...state.history, [...board]],
      redo: state.redo.slice(0, -1),
    })
  }

  // ---- 落格 ----
  const applyCell = (index: number, value: number) => {
    if (!board || state.won) return
    if (board[index] === value) return
    const next = [...board]
    next[index] = value
    setState({ ...state, board: next })
  }

  const cellMouseDown = (index: number, button: 0 | 2) => {
    if (!board || state.won) return
    pushHistory()
    const current = board[index]
    // 左键涂黑（黑→取消），右键标空（空→取消）；已定格不因拖动反复切换
    if (button === 0) {
      drag.current = { active: true, action: current === CELL_BLACK ? 'erase' : 'paint', touched: new Set() }
      if (current !== CELL_WHITE) applyCell(index, current === CELL_BLACK ? CELL_UNKNOWN : CELL_BLACK)
    } else {
      drag.current = { active: true, action: current === CELL_WHITE ? 'erase' : 'clear', touched: new Set() }
      if (current !== CELL_BLACK) applyCell(index, current === CELL_WHITE ? CELL_UNKNOWN : CELL_WHITE)
    }
    drag.current.touched.add(index)
    setSelected(index)
  }

  const cellMouseEnter = (index: number) => {
    if (!drag.current.active || !board) return
    if (drag.current.touched.has(index)) return
    drag.current.touched.add(index)
    const current = board[index]
    // 只作用于未定格
    if (current !== CELL_UNKNOWN) return
    if (drag.current.action === 'paint') applyCell(index, CELL_BLACK)
    else if (drag.current.action === 'clear') applyCell(index, CELL_WHITE)
    else if (drag.current.action === 'erase') applyCell(index, CELL_UNKNOWN)
  }

  useEffect(() => {
    const up = () => {
      drag.current.active = false
    }
    window.addEventListener('mouseup', up)
    return () => window.removeEventListener('mouseup', up)
  }, [])

  // 键盘：方向键移动 + 空格涂黑 + 回车标空
  const onKeyDown = (event: React.KeyboardEvent) => {
    if (!board || selected === null) return
    const row = Math.floor(selected / size)
    const col = selected % size
    if (event.key === 'ArrowUp' && row > 0) setSelected(selected - size)
    else if (event.key === 'ArrowDown' && row < size - 1) setSelected(selected + size)
    else if (event.key === 'ArrowLeft' && col > 0) setSelected(selected - 1)
    else if (event.key === 'ArrowRight' && col < size - 1) setSelected(selected + 1)
    else if (event.key === ' ') {
      event.preventDefault()
      pushHistory()
      applyCell(selected, board[selected] === CELL_BLACK ? CELL_UNKNOWN : CELL_BLACK)
    } else if (event.key === 'Enter') {
      event.preventDefault()
      pushHistory()
      applyCell(selected, board[selected] === CELL_WHITE ? CELL_UNKNOWN : CELL_WHITE)
    }
  }

  // ---- 检查 ----
  const check = async () => {
    if (!state.clues || !board) return
    try {
      const out = await callPlugin<CheckOut>(manifest.id, 'check', { clues: state.clues, board })
      setState({ ...state, rowDone: out.rowDone, colDone: out.colDone, rowConflict: out.rowConflict, colConflict: out.colConflict, won: out.done })
      if (out.done) toast.success(t('check.won'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  // ---- 提示：解释行/列候选重合并落格 ----
  const hint = async () => {
    if (!state.clues || !board || state.won) return
    try {
      const out = await callPlugin<HintOut>(manifest.id, 'hint', { clues: state.clues, board })
      if (out.found && out.row !== undefined && out.col !== undefined && out.black !== undefined) {
        pushHistory()
        const index = out.row * size + out.col
        const next = [...board]
        next[index] = out.black ? CELL_BLACK : CELL_WHITE
        setState({
          ...state,
          board: next,
          usedHint: true,
          rowDone: null,
          colDone: null,
        })
        setHintText(t('hint.found', { line: out.via === 0 ? t('hint.row') : t('hint.col'), number: out.via === 0 ? out.row + 1 : out.col + 1, row: out.row + 1, col: out.col + 1, value: out.black ? t('hint.black') : t('hint.white') }))
      } else {
        setHintText(t('hint.none'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const revealSolution = () => {
    setShowSolution(!showSolution)
    if (!showSolution) setState({ ...state, usedSolution: true })
  }

  // ---- 题纸导出 ----
  const exportPaper = async () => {
    if (!state.clues) return
    try {
      const out = await callPlugin<PaperOut>(manifest.id, 'export_paper', {
        clues: state.clues,
        solution: state.solution,
        title: paperTitle.trim() || `${t('paper.title')} ${size}×${size} ${localToday()}`,
      })
      const path = await host.dialog.saveFile('nonogram-paper.html', [{ name: 'HTML', extensions: ['html'] }])
      if (!path) return
      await host.fs.writeText(path, out.html)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  // ---- 渲染 ----
  const renderClues = (clues: Clues) => {
    // 顶部列线索
    const colHeads = clues.cols.map((clue, c) => (
      <div
        key={c}
        className={`flex flex-col items-center justify-end ${state.colDone?.[c] ? 'text-fg-muted' : ''} ${state.colConflict?.[c] ? 'text-fg-danger' : ''}`}
        style={{ minWidth: 22 }}
      >
        {clue.map((n, i) => (
          <span key={i} className={`text-[10px] leading-tight ${n === 0 ? 'opacity-40' : ''}`}>
            {n === 0 ? '0' : n}
          </span>
        ))}
      </div>
    ))
    return { colHeads }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel
        icon={<Sparkles />}
        title={t('game.title')}
        bodyClassName="space-y-2 p-3"
        actions={
          <div className="flex items-center gap-1">
            {SIZES.map((s) => (
              <Button key={s} size="sm" variant={state.size === s && state.clues?.size === s ? 'primary' : 'ghost'} disabled={pending} onClick={() => void newGame(s)}>
                {t('game.sizeLabel', { size: s })}
              </Button>
            ))}
          </div>
        }
      >
        <div className="flex flex-wrap items-center gap-2">
          <Tooltip content={t('game.undo')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('game.undo')} onClick={undo} disabled={state.history.length === 0}>
              <RotateCcw />
            </Button>
          </Tooltip>
          <Tooltip content={t('game.redo')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('game.redo')} onClick={redoStep} disabled={state.redo.length === 0}>
              <RotateCw />
            </Button>
          </Tooltip>
          <Tooltip content={t('game.check')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('game.check')} onClick={() => void check()} disabled={!board}>
              <ArrowLeftRight />
            </Button>
          </Tooltip>
          <Tooltip content={t('game.hint')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('game.hint')} onClick={() => void hint()} disabled={!board || state.won}>
              <Lightbulb />
            </Button>
          </Tooltip>
          <Tooltip content={t('game.solution')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('game.solution')} onClick={revealSolution} disabled={!state.solution}>
              {showSolution ? <Pause /> : <EyeIcon />}
            </Button>
          </Tooltip>
          {pending && <Spinner className="size-4" />}
          {(state.usedHint || state.usedSolution) && (
            <Badge variant="warning">{state.usedHint ? t('game.usedHint') : ''}{state.usedHint && state.usedSolution ? ' · ' : ''}{state.usedSolution ? t('game.usedSolution') : ''}</Badge>
          )}
          {state.won && <Badge variant="neutral">{t('check.solved')}</Badge>}
        </div>
        {hintText && <p className="text-xs text-fg-muted">{hintText}</p>}
      </Panel>

      <div className="min-h-0 flex-1 overflow-auto">
        {state.clues && board && (
          <div className="inline-flex flex-col gap-2" onKeyDown={onKeyDown} tabIndex={0}>
            <div className="flex">
              <div style={{ width: clueWidth(size) }} />
              <div className="flex">
                {renderClues(state.clues).colHeads}
              </div>
            </div>
            {Array.from({ length: size }).map((_, r) => (
              <div key={r} className="flex items-center">
                <div
                  className={`flex items-center justify-end gap-1 pr-2 ${state.rowDone?.[r] ? 'text-fg-muted' : ''} ${state.rowConflict?.[r] ? 'text-fg-danger' : ''}`}
                  style={{ width: clueWidth(size), minHeight: 22 }}
                >
                  {state.clues!.rows[r].map((n, i) => (
                    <span key={i} className={`text-[10px] leading-tight ${n === 0 ? 'opacity-40' : ''}`}>
                      {n === 0 ? '0' : n}
                    </span>
                  ))}
                </div>
                <div className="flex" role="grid" aria-label={t('game.board')}>
                  {Array.from({ length: size }).map((_, c) => {
                    const index = r * size + c
                    const value = board[index]
                    const isThickRight = (c + 1) % 5 === 0 && c !== size - 1
                    const isThickBottom = (r + 1) % 5 === 0 && r !== size - 1
                    return (
                      <button
                        key={c}
                        type="button"
                        role="gridcell"
                        aria-label={`${r + 1}-${c + 1}`}
                        onMouseDown={(event) => cellMouseDown(index, event.button === 2 ? 2 : 0)}
                        onMouseEnter={() => cellMouseEnter(index)}
                        onContextMenu={(event) => event.preventDefault()}
                        className={`relative border border-border-subtle ${isThickRight ? 'border-r-2 border-r-border' : ''} ${isThickBottom ? 'border-b-2 border-b-border' : ''} ${selected === index ? 'outline outline-2 outline-primary-500' : ''}`}
                        style={{ width: 22, height: 22, background: value === CELL_BLACK ? 'var(--color-fg, #111)' : value === CELL_WHITE ? 'transparent' : undefined }}
                      >
                        {value === CELL_WHITE && <span className="absolute inset-0 flex items-center justify-center text-fg-subtle"><Eraser className="size-3 opacity-50" /></span>}
                        {showSolution && state.solution?.[index] && value !== CELL_BLACK && (
                          <span className="absolute inset-0 flex items-center justify-center bg-primary-500/30" />
                        )}
                      </button>
                    )
                  })}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <Panel icon={<Download />} title={t('paper.title')} bodyClassName="space-y-2 p-3">
        <div className="flex items-center gap-2">
          <Input
            value={paperTitle}
            onChange={(event) => setPaperTitle(event.target.value)}
            placeholder={t('paper.namePlaceholder')}
            size="sm"
            aria-label={t('paper.namePlaceholder')}
            wrapperClassName="w-48"
          />
          <Button size="sm" variant="ghost" onClick={() => void exportPaper()} disabled={!state.clues}>
            <Download />
            {t('paper.export')}
          </Button>
          <span className="text-xs text-fg-subtle">{t('paper.hint')}</span>
        </div>
      </Panel>
    </div>
  )
}

function clueWidth(size: number): number {
  // 行线索区宽度随线索数量增长（15×15 最多 8 段）
  return size >= 15 ? 76 : size >= 10 ? 60 : 46
}

function EyeIcon() {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="size-4">
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  )
}
