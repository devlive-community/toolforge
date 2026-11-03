import { useState } from 'react'
import { Button, Input, Panel, Spinner } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, Grid3x3 } from 'lucide-react'

interface Cell {
  row: number
  column: number
  color: string
  symbol: string
}
interface Usage {
  color: string
  count: number
}
interface GenerateOut {
  columns: number
  rows: number
  cells: Cell[]
  usage: Usage[]
  totalBeads: number
  svg: string
}

export default function BeadPattern() {
  const { t, errorMessage } = usePlugin()
  const [path, setPath] = useState('')
  const [columns, setColumns] = useState(29) // tf-allow: 默认值
  const [rows, setRows] = useState(29) // tf-allow: 默认值
  const [maxColors, setMaxColors] = useState(12) // tf-allow: 默认值
  const args = path ? { path, columns, rows, maxColors } : null
  const generate = useDebouncedCall<GenerateOut>(args ? 'generate' : null, args, [path, columns, rows, maxColors])

  const pick = async () => {
    const picked = await host.dialog.openFile([{ name: 'PNG/JPEG', extensions: ['png', 'jpg', 'jpeg'] }])
    if (picked) setPath(picked)
  }

  const download = () => {
    if (!generate.result) return
    const blob = new Blob([generate.result.svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'bead-pattern.svg'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<Grid3x3 />} title={t('ui.gen')} bodyClassName="space-y-2.5 p-3">
        <Button onClick={() => void pick()}>{t('ui.pick_file')}</Button>
        <Input value={path} onChange={(event) => setPath(event.target.value)} placeholder={t('api.generate.path')} aria-label={t('api.generate.path')} />
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.columns')}
          <input type="range" min={4} max={100} value={columns} onChange={(event) => setColumns(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.columns')} />
          <span className="w-8 text-right tabular-nums">{columns}</span>
        </label>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.rows')}
          <input type="range" min={4} max={100} value={rows} onChange={(event) => setRows(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.rows')} />
          <span className="w-8 text-right tabular-nums">{rows}</span>
        </label>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.maxColors')}
          <input type="range" min={2} max={20} value={maxColors} onChange={(event) => setMaxColors(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.maxColors')} />
          <span className="w-8 text-right tabular-nums">{maxColors}</span>
        </label>
        {generate.result && (
          <div className="space-y-1">
            <p className="text-xs font-medium text-fg-muted">{t('usage.title')}: {t('usage.total', { count: generate.result.totalBeads })}</p>
            {generate.result.usage.map((usage) => (
              <p key={usage.color} className="flex justify-between text-xs">
                <span className="font-mono text-fg">{usage.color}</span>
                <span className="text-fg-muted tabular-nums">{usage.count}</span>
              </p>
            ))}
            <Button size="sm" variant="secondary" onClick={download}>
              <Download className="size-3.5" />
              {t('ui.export')}
            </Button>
          </div>
        )}
      </Panel>
      <Panel icon={<Grid3x3 />} title={t('ui.gen')} className="min-h-0" bodyClassName="flex items-center justify-center overflow-auto p-3" extra={generate.pending && <Spinner className="size-3.5" />}>
        {generate.error ? (
          <p className="text-xs text-danger">{errorMessage(generate.error)}</p>
        ) : (
          generate.result && (
            <img src={`data:image/svg+xml;utf8,${encodeURIComponent(generate.result.svg)}`} alt={t('ui.gen')} className="max-h-full" />
          )
        )}
      </Panel>
    </div>
  )
}
