import { useState } from 'react'
import {
  Badge,
  Button,
  CodeEditor,
  Empty,
  Input,
  Panel,
  Select,
  Spinner,
  toast,
} from '@toolforge/ui'
import { callPlugin, host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ListChecks, Plus, ShieldCheck, Trash2, TriangleAlert } from 'lucide-react'
import type { Diag, Exported, PolicyVerdict, Report, ResourceInput, ResourceVerdict } from './types'

const TYPES = ['script', 'style', 'image', 'font', 'connect', 'frame', 'worker', 'media', 'object', 'manifest'] as const

const SAMPLE_POLICIES =
  "default-src 'self'; script-src 'self' 'nonce-abc' https://cdn.example.com; img-src 'self' data:"

const SAMPLE_RESOURCES: ResourceInput[] = [
  { type: 'script', url: 'https://cdn.example.com/app.js' },
  { type: 'image', url: 'https://evil.example.net/track.gif' },
]

const STATUS_VARIANT: Record<ResourceVerdict['status'], 'success' | 'danger' | 'warning' | 'neutral'> = {
  allowed: 'success',
  blocked: 'danger',
  undetermined: 'warning',
  unrestricted: 'neutral',
}

function StatusBadge({ status }: { status: ResourceVerdict['status'] }) {
  const { t } = usePlugin()
  return <Badge variant={STATUS_VARIANT[status]}>{t(`status.${status}`)}</Badge>
}

function PolicyLine({ verdict }: { verdict: PolicyVerdict }) {
  const { t } = usePlugin()
  return (
    <span className="inline-flex items-center gap-2 rounded-control border border-border bg-surface-2 px-2 py-1 text-xs">
      <span className="text-fg-subtle">
        {t('result.policyIndex', { index: verdict.index + 1 })}
        {verdict.reportOnly ? ` · ${t('result.reportOnly')}` : ''}
      </span>
      <StatusBadge status={verdict.status} />
      {verdict.directive && <code className="font-mono text-fg-muted">{verdict.directive}</code>}
      {verdict.matched && <code className="font-mono text-fg-subtle">{verdict.matched}</code>}
    </span>
  )
}

function ResourceCard({ verdict }: { verdict: ResourceVerdict }) {
  const { t, errorMessage } = usePlugin()
  return (
    <div className="space-y-2 rounded-control border border-border bg-surface px-3 py-2">
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <StatusBadge status={verdict.status} />
        <Badge variant="neutral">{t(`types.${verdict.kind}`, { defaultValue: verdict.kind })}</Badge>
        <code className="min-w-0 flex-1 truncate font-mono text-fg" data-selectable title={verdict.url ?? undefined}>
          {verdict.url ?? t('result.inlineResource')}
        </code>
      </div>
      {verdict.directive && (
        <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
          <span>{t('result.directive')}</span>
          <code className="font-mono text-fg">{verdict.directive}</code>
          {verdict.matched ? (
            <>
              <span>{t('result.matched')}</span>
              <code className="font-mono text-fg">{verdict.matched}</code>
            </>
          ) : (
            <span className="text-fg-subtle">{t('result.viaNone')}</span>
          )}
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        {verdict.policies.map((policy) => (
          <PolicyLine key={policy.index} verdict={policy} />
        ))}
      </div>
      {verdict.reasons.map((reason, index) => (
        <p key={index} className="flex items-center gap-1.5 text-xs text-warning">
          <TriangleAlert className="size-3.5 shrink-0" />
          {errorMessage(reason)}
        </p>
      ))}
    </div>
  )
}

export default function CspInspector() {
  const { t, manifest, errorMessage } = usePlugin()
  const [policies, setPolicies] = useState(SAMPLE_POLICIES)
  const [documentUrl, setDocumentUrl] = useState('https://app.example.com/dashboard')
  const [resources, setResources] = useState<ResourceInput[]>(SAMPLE_RESOURCES)
  const [report, setReport] = useState<Report | null>(null)
  const [failure, setFailure] = useState<Diag | null>(null)
  const [busy, setBusy] = useState(false)

  const updateResource = (index: number, patch: Partial<ResourceInput>) => {
    setResources((prev) => prev.map((item, at) => (at === index ? { ...item, ...patch } : item)))
  }

  const inspect = async () => {
    setBusy(true)
    setFailure(null)
    try {
      const out = await callPlugin<Report & { handle: number }>(manifest.id, 'inspect', {
        policies,
        documentUrl,
        resources: resources.map((item) => ({
          type: item.type,
          url: item.url || null,
          nonce: item.nonce || null,
          hash: item.hash || null,
        })),
      })
      setReport(out)
    } catch (reason) {
      setFailure(reason as Diag)
    } finally {
      setBusy(false)
    }
  }

  const copyReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format })
      await host.clipboard.writeText(out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const saveReport = async (format: 'markdown' | 'json') => {
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format })
      const path = await host.dialog.saveFile(`csp-report.${out.ext}`, [
        { name: out.ext.toUpperCase(), extensions: [out.ext] },
      ])
      if (!path) return
      await host.fs.writeText(path, out.content)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(340px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<ShieldCheck />}
          title={t('input.policies')}
          footer={
            <div className="flex w-full items-center gap-2">
              <span className="shrink-0 text-xs text-fg-subtle">{t('input.document')}</span>
              <Input
                value={documentUrl}
                onChange={(event) => setDocumentUrl(event.target.value)}
                placeholder={t('input.documentPlaceholder')}
                wrapperClassName="min-w-0 flex-1"
                size="sm"
                aria-label={t('input.document')}
                spellCheck={false}
              />
              <Button size="sm" onClick={() => void inspect()} disabled={busy}>
                {busy && <Spinner className="size-3.5" />}
                {t('input.run')}
              </Button>
            </div>
          }
        >
          <div className="h-40 overflow-hidden rounded-control border border-border">
            <CodeEditor
              value={policies}
              onChange={setPolicies}
              language="text"
              placeholder={t('input.placeholder')}
              aria-label={t('input.policies')}
            />
          </div>
        </Panel>

        <Panel
          icon={<ListChecks />}
          title={t('input.resources')}
          bodyClassName="p-3"
          extra={
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setResources((prev) => [...prev, { type: 'script', url: '' }])}
            >
              <Plus />
              {t('input.add')}
            </Button>
          }
          footer={<span>{t('input.inline')}</span>}
        >
          <div className="space-y-2 overflow-y-auto">
            {resources.length === 0 && <p className="text-xs text-fg-subtle">{t('input.empty')}</p>}
            {resources.map((item, index) => (
              <div key={index} className="flex flex-wrap items-center gap-2">
                <Select<string>
                  value={item.type}
                  onValueChange={(value) => updateResource(index, { type: value })}
                  options={TYPES.map((type) => ({ value: type, label: t(`types.${type}`) }))}
                  size="sm"
                  aria-label={t('input.type')}
                />
                <Input
                  value={item.url ?? ''}
                  onChange={(event) => updateResource(index, { url: event.target.value })}
                  placeholder={t('input.urlPlaceholder')}
                  wrapperClassName="min-w-40 flex-1"
                  size="sm"
                  aria-label={t('input.url')}
                  spellCheck={false}
                />
                <Input
                  value={item.nonce ?? ''}
                  onChange={(event) => updateResource(index, { nonce: event.target.value })}
                  placeholder={t('input.nonce')}
                  wrapperClassName="w-28"
                  size="sm"
                  aria-label={t('input.nonce')}
                  spellCheck={false}
                />
                <Input
                  value={item.hash ?? ''}
                  onChange={(event) => updateResource(index, { hash: event.target.value })}
                  placeholder={t('input.hash')}
                  wrapperClassName="w-32"
                  size="sm"
                  aria-label={t('input.hash')}
                  spellCheck={false}
                />
                <Button
                  size="sm"
                  variant="ghost"
                  aria-label={t('input.remove')}
                  onClick={() => setResources((prev) => prev.filter((_, at) => at !== index))}
                >
                  <Trash2 />
                </Button>
              </div>
            ))}
          </div>
        </Panel>
      </div>

      <Panel
        icon={<ShieldCheck />}
        title={t('result.title')}
        extra={
          report && (
            <span className="flex items-center gap-1.5">
              <Badge variant="success">{`${t('status.allowed')} ${report.stats.allowed}`}</Badge>
              {report.stats.blocked > 0 && (
                <Badge variant="danger">{`${t('status.blocked')} ${report.stats.blocked}`}</Badge>
              )}
              {report.stats.undetermined > 0 && (
                <Badge variant="warning">{`${t('status.undetermined')} ${report.stats.undetermined}`}</Badge>
              )}
            </span>
          )
        }
        footer={
          <div className="flex w-full items-center gap-2">
            <Button size="sm" variant="soft" disabled={!report} onClick={() => void copyReport('markdown')}>
              {t('export.markdown')}
            </Button>
            <Button size="sm" variant="soft" disabled={!report} onClick={() => void copyReport('json')}>
              {t('export.json')}
            </Button>
            <span className="ml-auto flex items-center gap-2">
              <Button size="sm" disabled={!report} onClick={() => void saveReport('markdown')}>
                {t('export.saveMd')}
              </Button>
              <Button size="sm" disabled={!report} onClick={() => void saveReport('json')}>
                {t('export.saveJson')}
              </Button>
            </span>
          </div>
        }
      >
        <div className="h-full min-h-0 space-y-2 overflow-y-auto">
          {!report && !busy && (
            <div className="flex h-full items-center justify-center">
              <Empty icon={<ShieldCheck />} title={t('result.empty')} />
            </div>
          )}
          {failure && (
            <p className="flex items-center gap-1.5 px-1 py-2 text-xs text-danger">
              <TriangleAlert className="size-3.5" />
              {errorMessage(failure)}
            </p>
          )}
          {report?.diagnostics.map((diag, index) => (
            <p key={index} className="flex items-center gap-1.5 px-1 text-xs text-warning">
              <TriangleAlert className="size-3.5 shrink-0" />
              {errorMessage(diag)}
            </p>
          ))}
          {report?.policies.map((policy) => (
            <div key={policy.index} className="space-y-1 rounded-control border border-border bg-surface-2 px-3 py-2">
              <p className="flex items-center gap-2 text-xs">
                <span className="text-fg-subtle">{t('result.policyIndex', { index: policy.index + 1 })}</span>
                {policy.reportOnly && <Badge variant="info">{t('result.reportOnly')}</Badge>}
              </p>
              <code className="block font-mono text-xs break-all text-fg" data-selectable>
                {policy.raw}
              </code>
              {policy.diagnostics.map((diag, index) => (
                <p key={index} className="text-xs text-warning">
                  {errorMessage(diag)}
                </p>
              ))}
            </div>
          ))}
          {report?.resources.map((verdict) => (
            <ResourceCard key={verdict.index} verdict={verdict} />
          ))}
        </div>
      </Panel>
    </div>
  )
}
