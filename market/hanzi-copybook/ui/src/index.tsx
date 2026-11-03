import { useState } from 'react'
import { Button, Input, Panel, SegmentedControl, Select, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BookOpen, Printer } from 'lucide-react'

interface Cell {
  ch: string
  kind: string
}
interface Page {
  number: number
  rows: Cell[][]
}
interface LayoutOut {
  gridType: string
  perRow: number
  rowsPerPage: number
  pages: Page[]
  skipped: number
}

const GRIDS = ['tian', 'mi', 'grid'] as const
type Grid = (typeof GRIDS)[number]

export default function HanziCopybook() {
  const { t, errorMessage } = usePlugin()
  const [text, setText] = useState('永字八法 天行健君子以自强不息') // tf-allow: 默认值
  const [gridType, setGridType] = useState<Grid>('tian')
  const [perRow, setPerRow] = useState(10) // tf-allow: 默认值
  const [rowsPerPage, setRowsPerPage] = useState(6) // tf-allow: 默认值
  const [repeat, setRepeat] = useState(4) // tf-allow: 默认值
  const [mode, setMode] = useState<'trace' | 'empty'>('trace')
  const args = text.trim()
    ? { text, gridType, perRow, rowsPerPage, repeat, mode }
    : null
  const layout = useDebouncedCall<LayoutOut>(args ? 'layout' : null, args, [text, gridType, perRow, rowsPerPage, repeat, mode])

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<BookOpen />} title={t('ui.text')} bodyClassName="space-y-2.5 p-3">
        <Input value={text} onChange={(event) => setText(event.target.value)} placeholder={t('ui.text')} aria-label={t('ui.text')} />
        <SegmentedControl
          size="sm"
          value={gridType}
          onValueChange={(value) => setGridType(value as Grid)}
          aria-label={t('api.layout.gridType')}
          options={GRIDS.map((grid) => ({ value: grid, label: t(`grids.${grid}`) }))}
        />
        <Select value={mode} onValueChange={(value) => setMode(value as 'trace' | 'empty')} options={[{ value: 'trace', label: t('ui.trace') }, { value: 'empty', label: t('grids.grid') }]} aria-label={t('api.layout.mode')} />
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('api.layout.perRow')}
          <input type="range" min={4} max={16} value={perRow} onChange={(event) => setPerRow(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('api.layout.perRow')} />
          <span className="w-6 text-right tabular-nums">{perRow}</span>
        </label>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('api.layout.repeat')}
          <input type="range" min={1} max={10} value={repeat} onChange={(event) => setRepeat(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('api.layout.repeat')} />
          <span className="w-6 text-right tabular-nums">{repeat}</span>
        </label>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('api.layout.rowsPerPage')}
          <input type="range" min={2} max={20} value={rowsPerPage} onChange={(event) => setRowsPerPage(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('api.layout.rowsPerPage')} />
          <span className="w-6 text-right tabular-nums">{rowsPerPage}</span>
        </label>
        {layout.result && (
          <p className="text-xs text-fg-muted">
            {t('ui.pages', { count: layout.result.pages.length })}
            {layout.result.skipped > 0 && ` · ${t('ui.skipped', { count: layout.result.skipped })}`}
          </p>
        )}
        {layout.error && <p className="text-xs text-danger">{errorMessage(layout.error)}</p>}
        <Button size="sm" variant="secondary" onClick={() => window.print()}>
          <Printer className="size-3.5" />
          {t('ui.print')}
        </Button>
      </Panel>
      <Panel icon={<BookOpen />} title={t('ui.pages', { count: layout.result?.pages.length ?? 0 })} className="min-h-0" bodyClassName="min-h-0 overflow-auto space-y-4 p-3" extra={layout.pending && <Spinner className="size-3.5" />}>
        {(layout.result?.pages ?? []).map((page) => (
          <div key={page.number} className="rounded-card border border-border bg-white p-3 dark:bg-white">
            {page.rows.map((row, rowIndex) => (
              <div key={rowIndex} className="grid border-t border-l border-slate-300" style={{ gridTemplateColumns: `repeat(${layout.result?.perRow ?? 10}, 1fr)` }}>
                {row.map((cell, colIndex) => (
                  <div key={colIndex} className="relative aspect-square border-b border-r border-slate-300">
                    {gridType !== 'grid' && (
                      <svg viewBox="0 0 100 100" className="absolute inset-0 h-full w-full text-slate-300">
                        {gridType === 'tian' && (
                          <>
                            <line x1="50" y1="0" x2="50" y2="100" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                            <line x1="0" y1="50" x2="100" y2="50" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                          </>
                        )}
                        {gridType === 'mi' && (
                          <>
                            <line x1="50" y1="0" x2="50" y2="100" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                            <line x1="0" y1="50" x2="100" y2="50" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                            <line x1="0" y1="0" x2="100" y2="100" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                            <line x1="100" y1="0" x2="0" y2="100" stroke="currentColor" strokeWidth="1" strokeDasharray="4 3" />
                          </>
                        )}
                      </svg>
                    )}
                    {cell.kind !== 'empty' && (
                      <span
                        className={`absolute inset-0 flex items-center justify-center font-serif leading-none text-slate-900 ${
                          cell.kind === 'trace' ? 'opacity-25' : 'opacity-90'
                        }`}
                        style={{ fontSize: 'clamp(12px, 5cqw, 48px)' }}
                      >
                        {cell.ch}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            ))}
          </div>
        ))}
      </Panel>
    </div>
  )
}
