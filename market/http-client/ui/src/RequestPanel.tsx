import { useState, type ReactNode } from 'react'
import { CodeEditor, NumberInput, Panel, SegmentedControl, Switch, Tabs, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { AuthPanel, type AuthBinding } from './AuthPanel'
import { KeyValueEditor } from './KeyValueEditor'
import type { BodyKind, Pair, Request } from './types'

type RequestTab = 'params' | 'headers' | 'body' | 'auth' | 'settings'
const count = (pairs: Pair[]) => pairs.filter((p) => p.enabled && p.key.trim()).length
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

export function RequestPanel({ request, onChange, auth }: { request: Request; onChange: (request: Request) => void; auth: AuthBinding }) {
  const { t } = usePlugin()
  const [requestTab, setRequestTab] = useState<RequestTab>('params')
  const { params, headers, body, timeoutMs, followRedirects, useProxy, stream } = request
  const { kind: bodyKind, text: bodyText, form } = body
  const timeout = timeoutMs / 1000
  const setParams = (params: Pair[]) => onChange({ ...request, params })
  const setHeaders = (headers: Pair[]) => onChange({ ...request, headers })
  const setBodyKind = (kind: BodyKind) => onChange({ ...request, body: { ...body, kind } })
  const setBodyText = (text: string) => onChange({ ...request, body: { ...body, text } })
  const setForm = (form: Pair[]) => onChange({ ...request, body: { ...body, form } })
  const setTimeoutSeconds = (seconds: number) => onChange({ ...request, timeoutMs: seconds * 1000 })
  const setFollowRedirects = (followRedirects: boolean) => onChange({ ...request, followRedirects })
  const setUseProxy = (useProxy: boolean) => onChange({ ...request, useProxy })
  return (
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
                { value: 'auth', label: auth.effective ? t('request.authOn', { name: auth.effective.name }) : t('request.auth') },
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
          {requestTab === 'auth' && <AuthPanel binding={auth} request={request} />}
          {requestTab === 'settings' && (
            <div className="divide-y divide-border">
              <ToggleRow label={t('settings.timeout')} hint={stream ? t('settings.streamNoTimeout') : undefined}>
                <NumberInput size="sm" value={timeout} onValueChange={setTimeoutSeconds} min={1} max={600} className="w-32" aria-label={t('settings.timeout')} />
              </ToggleRow>
              <ToggleRow label={t('settings.redirects')} hint={stream ? t('settings.streamNotApplied') : undefined}>
                <Switch checked={followRedirects} onCheckedChange={setFollowRedirects} aria-label={t('settings.redirects')} />
              </ToggleRow>
              <ToggleRow label={t('settings.proxy')} hint={stream ? t('settings.streamNotApplied') : t('settings.proxyHint')}>
                <Switch checked={useProxy} onCheckedChange={setUseProxy} aria-label={t('settings.proxy')} />
              </ToggleRow>
            </div>
          )}
        </Panel>
  )
}
