import { Button } from '@toolforge/ui'
import { usePlugin, type AppError } from '@toolforge/plugin-ui-sdk'
import { Crosshair, TriangleAlert } from 'lucide-react'

export function ErrorCard({ error, onLocate }: { error: AppError; onLocate?: (line: number, column: number) => void }) {
  const { t, errorMessage } = usePlugin()
  const line = Number(error.params?.line ?? 0)
  const column = Number(error.params?.column ?? 0)
  const detail = typeof error.params?.detail === 'string' ? error.params.detail : null

  return (
    <div className="flex h-full items-start justify-center overflow-auto p-6">
      <div className="w-full max-w-md animate-fade-in rounded-card border border-danger/30 bg-danger-soft/60 p-4">
        <div className="flex items-center gap-2 text-danger">
          <TriangleAlert className="size-4" />
          <p className="text-[13px] font-semibold">{t('error.title')}</p>
        </div>
        <p className="mt-2 text-[13px] leading-relaxed text-fg" data-selectable>
          {errorMessage(error)}
        </p>
        {detail && (
          <p className="mt-2 rounded-control bg-surface/70 px-2.5 py-1.5 font-mono text-xs break-all text-fg-muted" data-selectable>
            {detail}
          </p>
        )}
        {line > 0 && onLocate && (
          <Button size="sm" className="mt-3" onClick={() => onLocate(line, column)}>
            <Crosshair />
            {t('error.locate')}
          </Button>
        )}
      </div>
    </div>
  )
}
