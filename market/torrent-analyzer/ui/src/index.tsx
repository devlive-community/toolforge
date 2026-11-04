import { useState } from 'react'
import { Button, Input, Panel, Spinner } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Magnet, ShieldCheck } from 'lucide-react'

interface TorrentFile {
  path: string
  size: number
}
interface AnalyzeOut {
  kind: string
  displayName: string
  infoHash: string
  pieceLength: number
  pieceCount: number
  totalSize: number
  files: TorrentFile[]
  trackers: string[]
  createdBy: string | null
  comment: string | null
}

function formatSize(bytes: number): string {
  if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(2)} GB`
  if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(2)} MB`
  if (bytes >= 1 << 10) return `${(bytes / (1 << 10)).toFixed(1)} KB`
  return `${bytes} B`
}

export default function TorrentAnalyzer() {
  const { t, errorMessage, call } = usePlugin()
  const [magnet, setMagnet] = useState('')
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<AnalyzeOut | null>(null)

  const pick = async () => {
    const picked = await host.dialog.openFile([{ name: 'Torrent', extensions: ['torrent'] }])
    if (!picked) return
    setBusy(true)
    try {
      setResult(await call<AnalyzeOut>('analyze', { source: picked }))
    } catch (error) {
      setResult(null)
      errorMessage(error as { code: string })
    } finally {
      setBusy(false)
    }
  }

  const analyzeMagnet = async () => {
    if (!magnet.trim()) return
    setBusy(true)
    try {
      setResult(await call<AnalyzeOut>('analyze', { source: magnet.trim() }))
    } catch {
      setResult(null)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <Panel icon={<Magnet />} title={t('api.analyze.summary')} bodyClassName="space-y-2.5 p-3">
        <Button onClick={() => void pick()}>{t('ui.pick')}</Button>
        <Input value={magnet} onChange={(event) => setMagnet(event.target.value)} placeholder={t('ui.magnetPlaceholder')} aria-label={t('ui.magnetPlaceholder')} />
        <Button size="sm" variant="secondary" onClick={() => void analyzeMagnet()} disabled={busy || !magnet.trim()}>
          {t('ui.analyze')}
        </Button>
        <p className="flex items-center gap-1.5 text-[11px] text-fg-subtle">
          <ShieldCheck className="size-3.5" />
          {t('ui.privacy')}
        </p>
      </Panel>
      <Panel icon={<Magnet />} title={result ? t(`ui.kind.${result.kind}`) : t('api.analyze.summary')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-1 p-3" extra={busy && <Spinner className="size-3.5" />}>
        {result ? (
          <>
            <p className="text-sm font-semibold text-fg">{result.displayName || '—'}</p>
            <p className="font-mono text-[11px] text-fg-muted">{t('ui.infohash')}: {result.infoHash}</p>
            {result.pieceCount > 0 && (
              <p className="text-xs text-fg-muted">{t('ui.pieceValue', { length: Math.round(result.pieceLength / 1024), count: result.pieceCount })} · {t('ui.total')}: {formatSize(result.totalSize)}</p>
            )}
            {result.trackers.length > 0 && (
              <div className="space-y-0.5">
                <p className="text-xs font-medium text-fg-muted">{t('ui.trackers')}</p>
                {result.trackers.map((tracker) => (
                  <p key={tracker} className="truncate font-mono text-[11px] text-fg-subtle">{tracker}</p>
                ))}
              </div>
            )}
            {result.files.length > 0 && (
              <div className="space-y-0.5">
                <p className="text-xs font-medium text-fg-muted">{t('ui.files')}</p>
                {result.files.slice(0, 50).map((file) => (
                  <p key={file.path} className="flex justify-between text-xs">
                    <span className="min-w-0 flex-1 truncate text-fg">{file.path}</span>
                    <span className="shrink-0 text-fg-muted tabular-nums">{formatSize(file.size)}</span>
                  </p>
                ))}
              </div>
            )}
            {(result.comment || result.createdBy) && (
              <p className="text-[11px] text-fg-subtle">{[result.createdBy, result.comment].filter(Boolean).join(' · ')}</p>
            )}
          </>
        ) : (
          <p className="text-xs text-fg-subtle">{t('ui.pick')}</p>
        )}
      </Panel>
    </div>
  )
}
