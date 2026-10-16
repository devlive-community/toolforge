import { useState, type MouseEvent } from 'react'
import { Badge, Button, Progress, Select, Spinner } from '@toolforge/ui'
import { usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Activity, Play, Square } from 'lucide-react'

interface Region {
  start: number
  end: number
  average: number
}

interface Report {
  block: number
  values: number[]
  overall: number
  regions: Region[]
  regionsTruncated: boolean
  threshold: number
}

const BLOCKS = ['256', '1024', '4096', '65536'] as const
const WIDTH = 1000
const HEIGHT = 80

const hex = (offset: number) => `0x${offset.toString(16).toUpperCase()}`

/** 熵曲线：横轴为文件偏移，纵轴 0–8 比特；点击跳转到对应偏移 */
export function Entropy({ path, size, cursor, onJump, onSelect }: { path: string; size: number; cursor: number; onJump: (offset: number) => void; onSelect: (from: number, length: number) => void }) {
  const { t, errorMessage } = usePlugin()
  const task = useTask<Report>()
  const [block, setBlock] = useState<(typeof BLOCKS)[number]>('4096')
  // 结果只属于分析时的文件
  const [analyzed, setAnalyzed] = useState<string | null>(null)
  const report = analyzed === path && task.status === 'succeeded' ? task.result : null

  const start = () => {
    setAnalyzed(path)
    void task.start('entropy', { path, block: Number(block) })
  }

  const values = report?.values ?? []
  const x = (index: number) => (values.length <= 1 ? 0 : (index / (values.length - 1)) * WIDTH)
  const y = (value: number) => HEIGHT - (value / 8) * HEIGHT
  const points = values.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(' ')
  const jump = (event: MouseEvent<SVGSVGElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width))
    onJump(Math.floor(ratio * Math.max(0, size - 1)))
  }
  const percent = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="space-y-2 border-b border-border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Activity className="size-4 text-fg-muted" />
        <span className="text-[13px] font-medium text-fg">{t('entropy.title')}</span>
        {report && <Badge>{t('entropy.overall', { value: report.overall.toFixed(2) })}</Badge>}
        {report && <span className="text-[11px] text-fg-subtle">{t('entropy.block', { size: report.block.toLocaleString() })}</span>}
        <div className="ml-auto flex items-center gap-2">
          <Select<(typeof BLOCKS)[number]>
            size="sm"
            value={block}
            onValueChange={setBlock}
            aria-label={t('entropy.blockSize')}
            options={BLOCKS.map((value) => ({ value, label: t(`entropy.blocks.${value}`) }))}
          />
          {task.running ? (
            <Button size="sm" onClick={task.cancel}>
              <Square />
              {t('entropy.cancel')}
            </Button>
          ) : (
            <Button size="sm" variant="primary" onClick={start}>
              <Play />
              {t('entropy.analyze')}
            </Button>
          )}
        </div>
      </div>
      {task.running ? (
        <div className="flex items-center gap-2">
          <Spinner className="size-3.5 text-fg-subtle" />
          <Progress className="flex-1" value={percent} />
        </div>
      ) : task.status === 'failed' && task.error && analyzed === path ? (
        <p className="text-xs text-danger">{errorMessage(task.error)}</p>
      ) : !report ? (
        <p className="text-xs text-fg-subtle">{t('entropy.hint')}</p>
      ) : (
        <>
          <svg
            viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
            preserveAspectRatio="none"
            className="h-20 w-full cursor-crosshair rounded-control bg-surface-2"
            onClick={jump}
            role="img"
            aria-label={t('entropy.chart')}
          >
            <polygon points={`0,${HEIGHT} ${points} ${WIDTH},${HEIGHT}`} className="fill-primary/15" />
            <polyline points={points} className="fill-none stroke-primary" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
            <line x1="0" x2={WIDTH} y1={y(report.threshold)} y2={y(report.threshold)} className="stroke-danger/60" strokeDasharray="6 4" vectorEffect="non-scaling-stroke" />
            {size > 1 && (
              <line x1={(cursor / (size - 1)) * WIDTH} x2={(cursor / (size - 1)) * WIDTH} y1="0" y2={HEIGHT} className="stroke-fg-muted" strokeWidth="1" vectorEffect="non-scaling-stroke" />
            )}
          </svg>
          {report.regions.length === 0 ? (
            <p className="text-[11px] text-fg-subtle">{t('entropy.noRegions', { threshold: report.threshold })}</p>
          ) : (
            <div className="space-y-1">
              <p className="text-[11px] text-fg-muted">{t('entropy.regions', { count: report.regions.length, threshold: report.threshold })}</p>
              <ul className="flex max-h-20 flex-wrap gap-1.5 overflow-auto">
                {report.regions.map((region) => (
                  <li key={region.start}>
                    <button
                      type="button"
                      onClick={() => onSelect(region.start, region.end - region.start)}
                      className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
                    >
                      {t('entropy.region', { start: hex(region.start), end: hex(region.end), value: region.average.toFixed(2) })}
                    </button>
                  </li>
                ))}
                {report.regionsTruncated && <li className="text-[11px] text-fg-subtle">{t('entropy.truncated')}</li>}
              </ul>
            </div>
          )}
        </>
      )}
    </div>
  )
}
