import { useRef } from 'react'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Button, Tooltip, cn } from '@toolforge/ui'
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp } from 'lucide-react'
import { mazeKey } from './keyboard'
import type { Op, Side, View } from './types'

export function Board({ view, disabled, action }: { view: View; disabled: boolean; action: (op: Op) => void }) {
  const { t } = usePlugin()
  const board = useRef<SVGSVGElement | null>(null)
  const move = (side: Side) => {
    if (!disabled) action({ type: 'move', side })
    board.current?.focus()
  }
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden rounded-card border border-border bg-surface p-3">
        <svg
          ref={board}
          tabIndex={0}
          role="application"
          aria-label={t('board.label')}
          aria-describedby="maze-keys"
          viewBox={`-0.45 -0.45 ${view.spec.cols + 0.9} ${view.spec.rows + 0.9}`}
          className="h-full min-h-0 w-full rounded-control outline-none focus-visible:ring-2 focus-visible:ring-ring"
          onKeyDown={(event) => {
            const direction = mazeKey(event)
            if (!direction) return
            event.preventDefault()
            if (direction !== 'consume' && !disabled) action({ type: 'move', side: direction })
          }}
        >
          {view.cells.map((cell) => (
            <rect
              key={cell.id}
              aria-hidden="true"
              x={cell.x}
              y={cell.y}
              width={1}
              height={1}
              className={cn('fill-transparent', !disabled && cell.adjacent && 'cursor-pointer hover:fill-primary-soft')}
              onClick={() => {
                if (!disabled) action({ type: 'moveTo', cell: cell.id })
                board.current?.focus()
              }}
            />
          ))}
          <path d={view.trailPath} className="stroke-primary/40" fill="none" strokeWidth={0.14} strokeLinecap="round" pointerEvents="none" />
          {view.answerPath && (
            <path
              d={view.answerPath}
              className="stroke-warning"
              fill="none"
              strokeWidth={0.12}
              strokeDasharray="0.18 0.14"
              strokeLinecap="round"
              pointerEvents="none"
            />
          )}
          <path d={view.wallPath} className="stroke-fg" fill="none" strokeWidth={0.06} strokeLinecap="square" pointerEvents="none" />
          <circle cx={view.start.x} cy={view.start.y} r={0.22} className="fill-surface stroke-fg-muted" strokeWidth={0.04} pointerEvents="none" />
          <text x={view.start.x} y={view.start.y + 0.1} textAnchor="middle" className="fill-fg-muted" fontSize={0.3} fontWeight="bold" pointerEvents="none">
            {t('board.startMark')}
          </text>
          <rect
            x={view.exit.x - 0.22}
            y={view.exit.y - 0.22}
            width={0.44}
            height={0.44}
            rx={0.05}
            className="fill-surface stroke-fg"
            strokeWidth={0.04}
            pointerEvents="none"
          />
          <text x={view.exit.x} y={view.exit.y + 0.1} textAnchor="middle" className="fill-fg" fontSize={0.3} fontWeight="bold" pointerEvents="none">
            {t('board.exitMark')}
          </text>
          {view.hint && (
            <circle
              cx={view.hint.x}
              cy={view.hint.y}
              r={0.3}
              className="fill-warning/20 stroke-warning"
              strokeWidth={0.05}
              strokeDasharray="0.12 0.08"
              pointerEvents="none"
            />
          )}
          <circle cx={view.player.x} cy={view.player.y} r={0.16} className="fill-primary stroke-primary-fg" strokeWidth={0.025} pointerEvents="none" />
        </svg>
      </div>
      <div className="flex shrink-0 items-center justify-between gap-3">
        <p id="maze-keys" className="max-w-sm text-[11px] leading-relaxed text-fg-muted">
          {t('board.keys')}
        </p>
        <div className="grid shrink-0 grid-cols-3 gap-1">
          {(
            [
              { side: 'top', Icon: ArrowUp, className: 'col-start-2' },
              { side: 'left', Icon: ArrowLeft, className: 'col-start-1 row-start-2' },
              { side: 'bottom', Icon: ArrowDown, className: 'col-start-2 row-start-2' },
              { side: 'right', Icon: ArrowRight, className: 'col-start-3 row-start-2' },
            ] as const
          ).map(({ side, Icon, className }) => (
            <Tooltip key={side} content={t(`move.${side}`)}>
              <Button size="icon-sm" className={className} disabled={disabled || view.finished} aria-label={t(`move.${side}`)} onClick={() => move(side)}>
                <Icon />
              </Button>
            </Tooltip>
          ))}
        </div>
      </div>
    </div>
  )
}
