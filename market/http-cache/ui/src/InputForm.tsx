import { useState } from 'react'
import { Button, Checkbox, CodeEditor, Input, Panel, SegmentedControl, Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Copy, FlaskConical, Play } from 'lucide-react'
import type { Scenario } from './types'
export function InputForm({
  cases,
  selected,
  compare,
  busy,
  select,
  change,
  copy,
  run,
  setCompare,
}: {
  cases: Scenario[]
  selected: number
  compare: boolean
  busy: boolean
  select: (n: number) => void
  change: (scenario: Scenario) => void
  copy: () => void
  run: () => void
  setCompare: (enabled: boolean) => void
}) {
  const { t } = usePlugin()
  const [headerTab, setHeaderTab] = useState<'requestHeaders' | 'responseHeaders' | 'newRequestHeaders'>('responseHeaders')
  const item = cases[selected]
  const update = (key: keyof Scenario, value: string | null) => change({ ...item, [key]: value })
  return (
    <Panel className="h-full min-h-0" title={t('input.title')} icon={<FlaskConical />} bodyClassName="flex h-full min-h-0 flex-col gap-3 p-3">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2">
        <div className={busy ? 'pointer-events-none opacity-50' : ''}>
          <SegmentedControl
            size="sm"
            value={String(selected)}
            onValueChange={(value) => select(value === '0' ? 0 : 1)}
            aria-label={t('input.case')}
            options={[
              { value: '0', label: t('input.baseline') },
              { value: '1', label: t('input.experiment') },
            ]}
          />
        </div>
        <Button size="sm" variant="ghost" disabled={busy} onClick={copy}>
          <Copy />
          {t('input.copy')}
        </Button>
      </div>
      <div className="shrink-0 space-y-2">
        <div className="grid grid-cols-2 gap-2">
          <label className="space-y-1">
            <span className="text-[11px] text-fg-muted">{t('input.name')}</span>
            <Input size="sm" disabled={busy} value={item.name} onChange={(e) => update('name', e.target.value)} maxLength={80} aria-label={t('input.name')} />
          </label>
          <label className="space-y-1">
            <span className="text-[11px] text-fg-muted">{t('input.role')}</span>
            <Select
              className="w-full"
              size="sm"
              value={item.role}
              onValueChange={(role) => change({ ...item, role })}
              disabled={busy}
              aria-label={t('input.role')}
              options={[
                { value: 'private', label: t('role.private') },
                { value: 'shared', label: t('role.shared') },
              ]}
            />
          </label>
        </div>
        <div className="grid grid-cols-2 gap-2">
          {(['method', 'status'] as const).map((key) => (
            <label key={key} className="space-y-1">
              <span className="text-[11px] text-fg-muted">{t(`input.${key}`)}</span>
              <Input
                size="sm"
                disabled={busy}
                value={item[key]}
                onChange={(e) => update(key, e.target.value)}
                maxLength={key === 'method' ? 32 : 3}
                aria-label={t(`input.${key}`)}
              />
            </label>
          ))}
        </div>
        <details open className="rounded-control border border-border">
          <summary className="cursor-pointer px-2 py-1 text-xs text-fg-muted">{t('input.timeTitle')}</summary>
          <div className="space-y-2 px-2 pb-2">
            <p className="text-[11px] leading-relaxed text-fg-subtle">{t('input.timeHint')}</p>
            {(['requestTime', 'responseTime', 'now'] as const).map((key) => (
              <label key={key} className="flex items-center gap-2">
                <span className="w-20 shrink-0 text-[11px] text-fg-muted">{t(`input.${key}`)}</span>
                <Input
                  size="sm"
                  wrapperClassName="min-w-0 flex-1"
                  disabled={busy}
                  value={item[key]}
                  onChange={(e) => update(key, e.target.value)}
                  maxLength={40}
                  aria-label={t(`input.${key}`)}
                  placeholder={t('input.missingTime')}
                />
              </label>
            ))}
          </div>
        </details>
      </div>
      <div className="shrink-0 space-y-2">
        <div className={busy ? 'pointer-events-none opacity-50' : ''}>
          <SegmentedControl
            size="sm"
            value={headerTab}
            onValueChange={setHeaderTab}
            aria-label={t('input.headerTabs')}
            options={[
              { value: 'requestHeaders', label: t('context.request') },
              { value: 'responseHeaders', label: t('context.response') },
              { value: 'newRequestHeaders', label: t('context.presented') },
            ]}
          />
        </div>
        {headerTab === 'newRequestHeaders' ? (
          <Checkbox
            disabled={busy}
            checked={item.newRequestHeaders !== null}
            onCheckedChange={(checked) => update('newRequestHeaders', checked ? item.requestHeaders : null)}
          >
            {t('input.useNewRequest')}
          </Checkbox>
        ) : (
          <p className="text-[11px] text-fg-subtle">{t('input.headerHint')}</p>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border">
        {headerTab === 'newRequestHeaders' && item.newRequestHeaders === null ? (
          <p className="p-4 text-xs leading-relaxed text-fg-muted">{t('input.sameRequest')}</p>
        ) : (
          <CodeEditor
            value={item[headerTab] ?? ''}
            onChange={(value) => update(headerTab, value)}
            readOnly={busy}
            language="text"
            aria-label={t(`headerLabel.${headerTab}`)}
          />
        )}
      </div>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border pt-3">
        <Checkbox disabled={busy} checked={compare} onCheckedChange={setCompare}>
          {t('input.compare')}
        </Checkbox>
        <Button size="sm" variant="primary" disabled={busy} onClick={run}>
          <Play />
          {t('input.run')}
        </Button>
      </div>
      <p className="shrink-0 text-[11px] text-fg-subtle">{t('input.transient')}</p>
    </Panel>
  )
}
