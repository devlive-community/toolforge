import { useEffect, useRef } from 'react'
import { Button, Input, Select, toast } from '@toolforge/ui'
import { useCopy, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Check, ClipboardPaste, Send, Square, Terminal } from 'lucide-react'
import { RequestPanel } from './RequestPanel'
import { ResponsePanel } from './ResponsePanel'
import { METHODS, type Method, type Reply, type Request, type Variable } from './types'

export function RequestEditor({ request, variables, onChange, onRunningChange, onImport, disabled }: { request: Request; variables: Variable[]; onChange: (request: Request) => void; onRunningChange: (running: boolean) => void; onImport: () => void; disabled: boolean }) {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Reply>()
  const { copy, copied } = useCopy()
  const starting = useRef(false)
  useEffect(() => { onRunningChange(task.running) }, [task.running, onRunningChange])
  const send = () => {
    if (!request.url.trim() || task.running || starting.current || disabled) return
    starting.current = true
    onRunningChange(true)
    void task.start('send', { ...request, variables }).finally(() => { starting.current = false })
  }
  const copyCurl = async () => {
    try { await copy(await call<string>('to_curl', { ...request, variables })) }
    catch (error) { toast.error(errorMessage(error)) }
  }
  return <div className="flex min-h-0 flex-1 flex-col gap-3" onKeyDown={(event) => { if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') { event.preventDefault(); send() } }}>
    <form className="flex shrink-0 flex-wrap items-center gap-2 rounded-card border border-border bg-surface p-3 shadow-card" onSubmit={(event) => { event.preventDefault(); send() }}>
      <Select<Method> value={request.method} onValueChange={(method) => onChange({ ...request, method })} options={METHODS.map((value) => ({ value, label: value }))} className="w-28 font-mono" aria-label={t('request.method')} />
      <Input value={request.url} onChange={(event) => onChange({ ...request, url: event.target.value })} className="font-mono" wrapperClassName="min-w-40 flex-1" placeholder={t('request.urlPlaceholder')} aria-label={t('request.url')} />
      <Button type="button" variant="outline" onClick={onImport} disabled={task.running || disabled}><ClipboardPaste />{t('import.short')}</Button>
      <Button type="button" variant="outline" onClick={() => void copyCurl()} disabled={!request.url.trim()}>{copied ? <Check className="text-success" /> : <Terminal />}{t('request.curl')}</Button>
      {task.running ? <Button type="button" onClick={() => void task.cancel()}><Square />{t('request.cancel')}</Button> : <Button type="submit" variant="primary" disabled={!request.url.trim() || disabled}><Send />{t('request.send')}</Button>}
    </form>
    <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(20rem,1fr)_minmax(20rem,1fr)] gap-3 overflow-auto xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] xl:grid-rows-[minmax(0,1fr)]">
      <RequestPanel request={request} onChange={onChange} />
      <ResponsePanel task={task} />
    </div>
  </div>
}
