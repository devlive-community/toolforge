import { Button, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { ImagePlus, Upload } from 'lucide-react'

export const EXTENSIONS = ['svg', 'png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tif', 'tiff']

export interface Info {
  path: string
  name: string
  width: number
  height: number
  svg: boolean
  square: boolean
  transparent: boolean
  small: boolean
}

export const dirname = (path: string) => path.replace(/[\\/][^\\/]*$/, '')

/** 源图片：已选时显示文件信息，未选时是可点击、可拖入的区域 */
export function SourceCard({ info, hovering, onPick, compact }: { info: Info | null; hovering: boolean; onPick: () => void; compact?: boolean }) {
  const { t } = usePlugin()
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{t('source.title')}</p>
      {info ? (
        <div className="flex items-center gap-2 rounded-control border border-border px-3 py-2">
          <div className="min-w-0 flex-1">
            <p className="truncate text-[13px] text-fg" title={info.path}>
              {info.path.split(/[\\/]/).pop()}
            </p>
            <p className="text-[11px] text-fg-subtle">{info.svg ? t('source.svg') : t('source.size', { width: info.width, height: info.height })}</p>
          </div>
          <Button size="icon-sm" variant="ghost" aria-label={t('source.change')} onClick={onPick}>
            <ImagePlus />
          </Button>
        </div>
      ) : (
        <button
          type="button"
          onClick={onPick}
          className={cn(
            'flex w-full flex-col items-center justify-center gap-1.5 rounded-card border-2 border-dashed px-3 text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            compact ? 'py-3' : 'py-6',
            hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          <Upload className="size-5" />
          <span className="text-[13px] font-medium">{t('source.drop')}</span>
          <span className="text-[11px] text-fg-subtle">{t('source.dropHint')}</span>
        </button>
      )}
      {info?.small && <p className="text-[11px] text-warning">{t('source.small', { size: Math.max(info.width, info.height) })}</p>}
      {info && !info.square && <p className="text-[11px] text-fg-subtle">{t('source.notSquare')}</p>}
    </div>
  )
}
