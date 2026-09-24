import { useState, type ReactNode } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, NumberInput, Panel, SegmentedControl, Select, Spinner, Switch, Tabs, cn, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, host, useCopy, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Check, Download, FileWarning, Info, Send, Square, Terminal, Upload } from 'lucide-react'
import { KeyValueEditor } from './KeyValueEditor'
import { METHODS, type BodyKind, type Method, type Pair, type Reply } from './types'

type RequestTab = 'params' | 'headers' | 'body' | 'settings'
type ResponseTab = 'body' | 'headers'

const count = (pairs: Pair[]) => pairs.filter((p) => p.enabled && p.key.trim()).length

function statusTone(status: number) {
  if (status < 300) return 'success' as const
  if (status < 400) return 'warning' as const
  return 'danger' as const
}

function ToggleRow({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-4 py-2">
      <span>
        <span className="block text-[13px] text-fg">{label}</span>
        {hint && <span className="block text-xs text-fg-subtle">{hint}</span>}
      </span>
      {children}
    </label>
  )
}

export function HttpClient() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Reply>()
  const { copy, copied } = useCopy()
  const [method, setMethod] = useState<Method>('GET')
  const [url, setUrl] = useState('https://api.github.com/repos/devlive-community/toolforge')
  const [params, setParams] = useState<Pair[]>([])
  const [headers, setHeaders] = useState<Pair[]>([{ key: 'Accept', value: 'application/json', enabled: true }])
  const [bodyKind, setBodyKind] = useState<BodyKind>('none')
  const [bodyText, setBodyText] = useState('{\n  \n}')
  const [form, setForm] = useState<Pair[]>([])
  const [timeout, setTimeoutSeconds] = useState(30)
  const [followRedirects, setFollowRedirects] = useState(true)
  const [useProxy, setUseProxy] = useState(true)
  const [requestTab, setRequestTab] = useState<RequestTab>('params')
  const [responseTab, setResponseTab] = useState<ResponseTab>('body')

  const request = () => ({
    method,
    url,
    params,
    headers,
    body: { kind: bodyKind, text: bodyText, form },
    timeoutMs: timeout * 1000,
    followRedirects,
    useProxy,
  })

  const send = () => {
    if (!url.trim() || task.running) return
    task.start('send', request())
  }

  const copyCurl = async () => {
    try {
      await copy(await call<string>('to_curl', request()))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const reply = task.result
  const saveBody = async () => {
    if (!reply) return
    try {
      const name = reply.url.split('?')[0].split('/').filter(Boolean).pop() || 'response'
      const path = await host.dialog.saveFile(reply.kind === 'json' && !name.includes('.') ? `${name}.json` : name)
      if (!path) return
      await call('save_body', { id: reply.id, path })
      toast.success(t('common:saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const language = reply?.kind === 'json' ? 'json' : reply?.contentType && /xml|html/i.test(reply.contentType) ? 'xml' : 'text'

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <form
        className="flex items-center gap-2 rounded-card border border-border bg-surface p-3 shadow-card"
        onSubmit={(event) => {
          event.preventDefault()
          send()
        }}
      >
        <Select<Method>
          value={method}
          onValueChange={setMethod}
          className="w-32 font-mono"
          aria-label={t('request.method')}
          options={METHODS.map((value) => ({ value, label: value }))}
        />
        <Input
          value={url}
          onChange={(event) => setUrl(event.target.value)}
          className="font-mono"
          wrapperClassName="flex-1"
          placeholder={t('request.urlPlaceholder')}
          aria-label={t('request.url')}
        />
        <Button type="button" variant="outline" onClick={copyCurl} disabled={!url.trim()}>
          {copied ? <Check className="text-success" /> : <Terminal />}
          {t('request.curl')}
        </Button>
        {task.running ? (
          <Button type="button" onClick={task.cancel}>
            <Square />
            {t('request.cancel')}
          </Button>
        ) : (
          <Button type="submit" variant="primary" disabled={!url.trim()}>
            <Send />
            {t('request.send')}
          </Button>
        )}
      </form>

      <div className="grid min-h-0 grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)] gap-3">
        <Panel
          title={
            <Tabs<RequestTab>
              variant="underline"
              value={requestTab}
              onValueChange={setRequestTab}
              aria-label={t('request.title')}
              items={[
                { value: 'params', label: t('request.params', { count: count(params) }) },
                { value: 'headers', label: t('request.headers', { count: count(headers) }) },
                { value: 'body', label: t('request.body') },
                { value: 'settings', label: t('request.settings') },
              ]}
            />
          }
          bodyClassName={cn('min-h-0', requestTab === 'body' && bodyKind !== 'none' && bodyKind !== 'form' ? 'flex flex-col' : 'overflow-auto p-3')}
        >
          {requestTab === 'params' && <KeyValueEditor pairs={params} onChange={setParams} keyPlaceholder={t('request.paramKey')} />}
          {requestTab === 'headers' && <KeyValueEditor pairs={headers} onChange={setHeaders} keyPlaceholder={t('request.headerKey')} />}
          {requestTab === 'body' && (
            <>
              <div className={cn('shrink-0', bodyKind === 'json' || bodyKind === 'text' ? 'border-b border-border p-3' : 'mb-3')}>
                <SegmentedControl<BodyKind>
                  size="sm"
                  value={bodyKind}
                  onValueChange={setBodyKind}
                  aria-label={t('request.body')}
                  options={[
                    { value: 'none', label: t('bodyKinds.none') },
                    { value: 'json', label: 'JSON' },
                    { value: 'text', label: t('bodyKinds.text') },
                    { value: 'form', label: t('bodyKinds.form') },
                  ]}
                />
              </div>
              {(bodyKind === 'json' || bodyKind === 'text') && (
                <div className="min-h-0 flex-1">
                  <CodeEditor value={bodyText} onChange={setBodyText} language={bodyKind === 'json' ? 'json' : 'text'} aria-label={t('request.body')} />
                </div>
              )}
              {bodyKind === 'form' && <KeyValueEditor pairs={form} onChange={setForm} keyPlaceholder={t('request.fieldKey')} />}
              {bodyKind === 'none' && <p className="text-xs text-fg-subtle">{t('request.noBody')}</p>}
            </>
          )}
          {requestTab === 'settings' && (
            <div className="divide-y divide-border">
              <ToggleRow label={t('settings.timeout')}>
                <NumberInput size="sm" value={timeout} onValueChange={setTimeoutSeconds} min={1} max={600} className="w-32" aria-label={t('settings.timeout')} />
              </ToggleRow>
              <ToggleRow label={t('settings.redirects')}>
                <Switch checked={followRedirects} onCheckedChange={setFollowRedirects} aria-label={t('settings.redirects')} />
              </ToggleRow>
              <ToggleRow label={t('settings.proxy')} hint={t('settings.proxyHint')}>
                <Switch checked={useProxy} onCheckedChange={setUseProxy} aria-label={t('settings.proxy')} />
              </ToggleRow>
            </div>
          )}
        </Panel>

        <Panel
          title={
            <Tabs<ResponseTab>
              variant="underline"
              value={responseTab}
              onValueChange={setResponseTab}
              aria-label={t('response.title')}
              items={[
                { value: 'body', label: t('response.body') },
                { value: 'headers', label: t('response.headers', { count: reply?.headers.length ?? 0 }) },
              ]}
            />
          }
          extra={task.running && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            reply && (
              <>
                <Badge variant={statusTone(reply.status)}>{`${reply.status} ${reply.statusText}`}</Badge>
                <span className="text-xs text-fg-muted tabular-nums">{t('response.meta', { ms: reply.elapsedMs, size: formatBytes(reply.size) })}</span>
                {reply.kind !== 'binary' && <CopyButton text={reply.body} />}
                <Button size="icon-sm" variant="ghost" aria-label={t('response.save')} onClick={saveBody}>
                  <Download />
                </Button>
              </>
            )
          }
          footer={
            reply && (
              <>
                <span className="truncate" title={reply.url}>
                  {reply.url}
                </span>
                {reply.redirects.length > 0 && <span className="shrink-0">{t('response.redirects', { count: reply.redirects.length })}</span>}
                <span className="ml-auto shrink-0">{reply.version}</span>
              </>
            )
          }
          bodyClassName="flex min-h-0 flex-col"
        >
          {task.status === 'failed' && task.error ? (
            <div className="m-auto max-w-md space-y-1 p-6 text-center">
              <FileWarning className="mx-auto size-6 text-danger" />
              <p className="text-[13px] text-danger">{errorMessage(task.error)}</p>
            </div>
          ) : !reply ? (
            <Empty icon={task.running ? <Spinner /> : <Upload />} title={task.running ? t('response.waiting') : t('response.empty')} description={task.running ? undefined : t('response.emptyHint')} />
          ) : responseTab === 'headers' ? (
            <table className="w-full text-left text-xs">
              <tbody className="font-mono">
                {reply.headers.map(([name, value], index) => (
                  <tr key={`${name}-${index}`} className="border-b border-border align-top">
                    <td className="w-1/3 px-3 py-2 text-fg-muted" data-selectable>
                      {name}
                    </td>
                    <td className="px-3 py-2 break-all text-fg" data-selectable>
                      {value}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : reply.kind === 'binary' ? (
            <Empty
              icon={<Download />}
              title={t('response.binary', { size: formatBytes(reply.size) })}
              description={reply.contentType ?? undefined}
              action={
                <Button onClick={saveBody}>
                  <Download />
                  {t('response.save')}
                </Button>
              }
            />
          ) : (
            <>
              {reply.truncated && (
                <p className="flex shrink-0 items-center gap-2 border-b border-border bg-warning-soft/60 px-3 py-2 text-xs text-warning">
                  <Info className="size-3.5" />
                  {t('response.truncated', { size: formatBytes(reply.size) })}
                </p>
              )}
              <div className="min-h-0 flex-1">
                <CodeEditor value={reply.body} readOnly language={language} lineWrapping aria-label={t('response.body')} />
              </div>
            </>
          )}
        </Panel>
      </div>
    </div>
  )
}
