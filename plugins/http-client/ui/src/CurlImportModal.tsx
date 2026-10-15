import { useRef, useState } from 'react'
import { Button, CodeEditor, Modal } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { AlertTriangle, Terminal } from 'lucide-react'
import type { ImportedCurl, Request } from './types'

export function CurlImportModal({ open, onClose, onImport }: { open: boolean; onClose: () => void; onImport: (request: Request) => Promise<boolean> }) {
  const { t, call, errorMessage } = usePlugin()
  const [command, setCommand] = useState('')
  const [result, setResult] = useState<ImportedCurl | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const sequence = useRef(0)
  const close = () => { sequence.current++; setBusy(false); setResult(null); setError(null); setCommand(''); onClose() }
  const parse = async () => {
    const seq = ++sequence.current
    setBusy(true); setError(null)
    try { const imported = await call<ImportedCurl>('import_curl', { command }); if (seq === sequence.current) setResult(imported) }
    catch (cause) { if (seq === sequence.current) setError(errorMessage(cause)) }
    finally { if (seq === sequence.current) setBusy(false) }
  }
  return <Modal open={open} onOpenChange={(value) => !value && close()} title={t('import.title')} description={t('import.hint')} size="lg" closeLabel={t('common:close')} footer={<>
    <Button variant="ghost" onClick={close}>{t('common:cancel')}</Button>
    {result ? <Button variant="primary" disabled={busy} onClick={() => { setBusy(true); void onImport(result.request).then((created) => { if (created) close() }).finally(() => setBusy(false)) }}>{t('import.apply')}</Button> : <Button variant="primary" disabled={busy || !command.trim()} onClick={() => void parse()}><Terminal />{t('import.parse')}</Button>}
  </>}>
    <div className="space-y-3 px-5 pb-5">
      <div className="h-48 overflow-hidden rounded-control border border-border"><CodeEditor value={command} onChange={(value) => { sequence.current++; setBusy(false); setCommand(value); setResult(null); setError(null) }} language="text" placeholder={t('import.placeholder')} aria-label={t('import.command')} /></div>
      {error && <p className="text-xs text-danger" role="alert">{error}</p>}
      {result && <div className="space-y-2">
        <p className="break-all font-mono text-xs text-fg">{result.request.method} {result.request.url}</p>
        {result.warnings.length ? <div role="status" className="space-y-2 rounded-control bg-warning-soft/60 p-3 text-xs text-warning">{result.warnings.map((warning, index) => <p key={index} className="flex items-start gap-2"><AlertTriangle className="size-3.5 shrink-0" /><span>{t(`warnings.${warning.code}`, { ...warning.params, defaultValue: t('import.genericWarning') })}</span></p>)}</div> : <p className="text-xs text-success">{t('import.ready')}</p>}
      </div>}
    </div>
  </Modal>
}
