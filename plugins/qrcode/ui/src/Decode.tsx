import { useEffect, useState } from 'react'
import { Badge, Button, Empty, Panel, Spinner, cn } from '@toolforge/ui'
import { CopyButton, host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ImageUp, ScanLine, Upload } from 'lucide-react'
import type { Decoded } from './types'

const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp']
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path

export function Decode() {
  const { t, call, errorMessage } = usePlugin()
  const [path, setPath] = useState<string | null>(null)
  const [results, setResults] = useState<Decoded[] | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)
  const [hovering, setHovering] = useState(false)

  const scan = async (file: string) => {
    setPath(file)
    setBusy(true)
    setError(null)
    setResults(null)
    try {
      setResults(await call<Decoded[]>('decode', { path: file }))
    } catch (reason) {
      setError(reason)
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && scan(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
    // scan 只依赖稳定的 call 与 setState，挂载时注册一次即可
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const pick = async () => {
    try {
      const file = await host.dialog.openFile([{ name: t('decode.filter'), extensions: IMAGE_EXTENSIONS }])
      if (file) await scan(file)
    } catch (reason) {
      setError(reason)
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel
        icon={<ImageUp />}
        title={t('decode.image')}
        actions={
          path && (
            <Button size="sm" onClick={pick}>
              {t('decode.another')}
            </Button>
          )
        }
        bodyClassName="p-3"
      >
        <button
          type="button"
          onClick={pick}
          className={cn(
            'flex h-full w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed p-4 text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          <Upload className="size-6" />
          <span className="text-[13px] font-medium">{path ? baseName(path) : t('decode.drop')}</span>
          <span className="text-xs text-fg-subtle">{t('decode.dropHint')}</span>
        </button>
      </Panel>

      <Panel
        icon={<ScanLine />}
        title={t('decode.results')}
        extra={busy ? <Spinner className="size-3.5 text-fg-subtle" /> : results && <Badge variant="success">{t('decode.found', { count: results.length })}</Badge>}
        bodyClassName="overflow-auto p-2"
      >
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !results ? (
          <Empty icon={<ScanLine />} title={busy ? t('decode.scanning') : t('decode.empty')} />
        ) : (
          <div className="space-y-2">
            {results.map((result, index) => (
              <section key={index} className="rounded-card border border-border">
                <header className="flex items-center gap-2 border-b border-border px-3 py-1.5 text-xs text-fg-muted">
                  <span>{t('decode.meta', { version: result.version, ecc: result.ecc })}</span>
                  <CopyButton text={result.text} className="ml-auto" />
                </header>
                <p className="px-3 py-2 font-mono text-[13px] break-all whitespace-pre-wrap text-fg" data-selectable>
                  {result.text}
                </p>
              </section>
            ))}
          </div>
        )}
      </Panel>
    </div>
  )
}
