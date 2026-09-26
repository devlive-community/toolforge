import { useRef, useState, type PointerEvent } from 'react'
import { Button, SegmentedControl, Spinner, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronsLeftRight } from 'lucide-react'
import type { FileResult } from './types'

export interface CompareImages {
  original: string
  output: string
}

type Zoom = 'fit' | 'actual'

/** 透明区域显示为棋盘格 */
const CHECKER =
  'bg-[conic-gradient(var(--color-border)_25%,var(--color-surface)_0_50%,var(--color-border)_0_75%,var(--color-surface)_0)] bg-[length:14px_14px]'

/** 原图与压缩结果叠放，拖动分隔线对比 */
export function CompareView({ file, images }: { file: FileResult; images: CompareImages | null }) {
  const { t } = usePlugin()
  const [split, setSplit] = useState(50)
  const [zoom, setZoom] = useState<Zoom>('fit')
  const frame = useRef<HTMLDivElement>(null)
  const dragging = useRef(false)

  const move = (event: PointerEvent) => {
    if (!dragging.current || !frame.current) return
    const rect = frame.current.getBoundingClientRect()
    setSplit(Math.min(100, Math.max(0, ((event.clientX - rect.left) / rect.width) * 100)))
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 px-5 pb-5">
      <div className="flex flex-wrap items-center gap-3 text-xs text-fg-muted tabular-nums">
        <span>{t('compare.original', { size: formatBytes(file.beforeBytes) })}</span>
        <ChevronsLeftRight className="size-3.5" />
        <span className="font-medium text-fg">{t('compare.compressed', { size: formatBytes(file.afterBytes) })}</span>
        <SegmentedControl<Zoom>
          size="sm"
          className="ml-auto"
          value={zoom}
          onValueChange={setZoom}
          aria-label={t('compare.zoom')}
          options={[
            { value: 'fit', label: t('compare.fit') },
            { value: 'actual', label: t('compare.actual') },
          ]}
        />
        <Button size="sm" variant="ghost" onClick={() => setSplit(50)}>
          {t('compare.center')}
        </Button>
      </div>
      <div className={cn('min-h-0 flex-1 rounded-control border border-border', CHECKER, zoom === 'fit' ? 'flex items-center justify-center overflow-hidden' : 'overflow-auto')}>
        {images ? (
          <div
            ref={frame}
            className={cn('relative touch-none select-none', zoom === 'fit' ? 'max-h-full max-w-full' : 'w-max')}
            onPointerDown={(event) => {
              dragging.current = true
              event.currentTarget.setPointerCapture(event.pointerId)
              move(event)
            }}
            onPointerMove={move}
            onPointerUp={() => (dragging.current = false)}
          >
            <img src={images.original} alt={t('compare.originalLabel')} draggable={false} className={cn('block', zoom === 'fit' ? 'max-h-[60vh] max-w-full object-contain' : 'max-w-none')} />
            <img
              src={images.output}
              alt={t('compare.compressedLabel')}
              draggable={false}
              className="absolute inset-0 block size-full max-w-none object-contain"
              style={{ clipPath: `inset(0 0 0 ${split}%)` }}
            />
            <div className="pointer-events-none absolute inset-y-0 w-0.5 -translate-x-1/2 bg-primary shadow-card" style={{ left: `${split}%` }}>
              <span className="absolute top-1/2 left-1/2 flex size-7 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full bg-primary text-fg-on-primary shadow-card">
                <ChevronsLeftRight className="size-4" />
              </span>
            </div>
            <span className="pointer-events-none absolute top-2 left-2 rounded-control bg-overlay px-2 py-0.5 text-[11px] font-medium text-fg-on-primary">{t('compare.originalLabel')}</span>
            <span className="pointer-events-none absolute top-2 right-2 rounded-control bg-overlay px-2 py-0.5 text-[11px] font-medium text-fg-on-primary">{t('compare.compressedLabel')}</span>
          </div>
        ) : (
          <Spinner />
        )}
      </div>
    </div>
  )
}
