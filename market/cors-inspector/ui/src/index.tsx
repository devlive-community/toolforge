import { useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, Panel, Select, Spinner, Tooltip, toast } from '@toolforge/ui'
import { CopyButton, callPlugin, host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CornerDownLeft, CornerUpRight, Globe, Plus, ShieldCheck, Trash2 } from 'lucide-react'
import type { Diag, Exported, HeaderInput, Report } from './types'

/** 敏感头的值默认遮罩展示，与后端 HeaderView 的规则一致 */
const SENSITIVE = ['cookie', 'cookie2', 'authorization', 'proxy-authorization', 'set-cookie']

const STAGE_VARIANT = { allowed: 'success', blocked: 'danger', undetermined: 'warning', skipped: 'neutral' } as const
const CONDITION_VARIANT = { pass: 'success', fail: 'danger', unknown: 'warning' } as const
const OVERALL_VARIANT = { allowed: 'success', blocked: 'danger', undetermined: 'warning' } as const

interface ResponseBlock {
  status: string
  headers: HeaderInput[]
}

const emptyBlock = (): ResponseBlock => ({ status: '', headers: [] })

/** 响应块全部为空时传 null，让后端区分「未填写」与「已捕获的空响应」 */
function toResponse(block: ResponseBlock) {
  const filled = block.status !== '' || block.headers.some((item) => item.name.trim() || item.value.trim())
  if (!filled) return null
  return {
    status: block.status === '' ? null : Number(block.status),
    headers: block.headers.filter((item) => item.name.trim()),
  }
}

function ConditionRow({ condition }: { condition: Report['preflight']['conditions'][number] }) {
  const { t, errorMessage } = usePlugin()
  return (
    <li className="flex flex-wrap items-center gap-2 text-xs">
      <Badge variant={CONDITION_VARIANT[condition.status]}>{t(`status.${condition.status}`)}</Badge>
      <span className="font-medium text-fg">{t(`conditions.${condition.id}`)}</span>
      {condition.detail && <code className="max-w-full truncate font-mono text-fg-subtle">{condition.detail}</code>}
      {condition.reason && <span className="text-fg-muted">{errorMessage(condition.reason)}</span>}
    </li>
  )
}

function HeaderRows({
  items,
  onChange,
  removeLabel,
}: {
  items: HeaderInput[]
  onChange: (items: HeaderInput[]) => void
  removeLabel: string
}) {
  const { t } = usePlugin()
  return (
    <div className="space-y-2">
      {items.map((item, index) => {
        const sensitive = SENSITIVE.includes(item.name.trim().toLowerCase())
        return (
          <div key={index} className="flex items-center gap-2">
            <Input
              value={item.name}
              onChange={(event) => onChange(items.map((row, at) => (at === index ? { ...row, name: event.target.value } : row)))}
              placeholder={t('input.headerName')}
              wrapperClassName="w-44 shrink-0"
              size="sm"
              aria-label={t('input.headerName')}
              spellCheck={false}
            />
            <Input
              type={sensitive ? 'password' : undefined}
              value={item.value}
              onChange={(event) => onChange(items.map((row, at) => (at === index ? { ...row, value: event.target.value } : row)))}
              placeholder={t('input.headerValue')}
              wrapperClassName="min-w-0 flex-1"
              size="sm"
              aria-label={t('input.headerValue')}
              spellCheck={false}
            />
            <Tooltip content={removeLabel}>
              <Button
                size="icon-sm"
                variant="ghost"
                aria-label={removeLabel}
                onClick={() => onChange(items.filter((_, at) => at !== index))}
              >
                <Trash2 />
              </Button>
            </Tooltip>
          </div>
        )
      })}
      <Button size="sm" variant="ghost" onClick={() => onChange([...items, { name: '', value: '' }])}>
        <Plus />
        {t('input.add')}
      </Button>
    </div>
  )
}

function ResponseFields({
  title,
  icon,
  block,
  onChange,
}: {
  title: string
  icon: ReactNode
  block: ResponseBlock
  onChange: (block: ResponseBlock) => void
}) {
  const { t } = usePlugin()
  return (
    <Panel title={title} icon={icon} bodyClassName="space-y-2 p-3">
      <label className="flex items-center gap-2 text-xs text-fg-muted">
        <span className="shrink-0">{t('input.status')}</span>
        <Input
          value={block.status}
          onChange={(event) => onChange({ ...block, status: event.target.value.replace(/[^\d]/g, '') })}
          placeholder={t('input.statusPlaceholder')}
          inputMode="numeric"
          wrapperClassName="w-32"
          size="sm"
          aria-label={t('input.status')}
        />
      </label>
      <HeaderRows
        items={block.headers}
        onChange={(headers) => onChange({ ...block, headers })}
        removeLabel={t('input.remove')}
      />
    </Panel>
  )
}

export default function CorsInspector() {
  const { t, manifest, errorMessage } = usePlugin()
  const [documentUrl, setDocumentUrl] = useState('https://app.example.com')
  const [targetUrl, setTargetUrl] = useState('https://api.example.com/v1/data')
  const [method, setMethod] = useState('GET')
  const [credentials, setCredentials] = useState('same-origin')
  const [headers, setHeaders] = useState<HeaderInput[]>([{ name: 'Content-Type', value: 'application/json' }])
  const [preflight, setPreflight] = useState<ResponseBlock>(emptyBlock)
  const [actual, setActual] = useState<ResponseBlock>(emptyBlock)
  const [report, setReport] = useState<Report | null>(null)
  const [failure, setFailure] = useState<Diag | null>(null)
  const [busy, setBusy] = useState(false)

  const inspect = async () => {
    setBusy(true)
    setFailure(null)
    try {
      const out = await callPlugin<Report & { handle: number }>(manifest.id, 'inspect', {
        documentUrl,
        targetUrl,
        method,
        credentials,
        headers: headers.filter((item) => item.name.trim()),
        preflight: toResponse(preflight),
        actual: toResponse(actual),
      })
      setReport(out)
    } catch (reason) {
      setFailure(reason as Diag)
    } finally {
      setBusy(false)
    }
  }

  const exportReport = async (format: 'markdown' | 'json', save: boolean) => {
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format })
      if (save) {
        const path = await host.dialog.saveFile(`cors-report.${out.ext}`, [
          { name: out.ext.toUpperCase(), extensions: [out.ext] },
        ])
        if (!path) return
        await host.fs.writeText(path, out.content)
        toast.success(t('common:saved'))
      } else {
        await host.clipboard.writeText(out.content)
        toast.success(t('common:saved'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const request = report?.request
  const outcome = report ? t(`outcome.${report.outcome}`) : ''

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(360px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        <Panel
          icon={<Globe />}
          title={t('input.request')}
          bodyClassName="space-y-2 p-3"
          footer={
            <div className="flex w-full items-center gap-3">
              <Button size="sm" variant="primary" onClick={() => void inspect()} disabled={busy}>
                {busy && <Spinner className="size-3.5" />}
                {t('input.run')}
              </Button>
              <span className="truncate text-[11px] text-fg-subtle">{t('input.hint')}</span>
            </div>
          }
        >
          <label className="block space-y-1 text-xs text-fg-muted">
            <span>{t('input.document')}</span>
            <Input
              value={documentUrl}
              onChange={(event) => setDocumentUrl(event.target.value)}
              placeholder={t('input.documentPlaceholder')}
              size="sm"
              aria-label={t('input.document')}
              spellCheck={false}
            />
          </label>
          <label className="block space-y-1 text-xs text-fg-muted">
            <span>{t('input.target')}</span>
            <Input
              value={targetUrl}
              onChange={(event) => setTargetUrl(event.target.value)}
              placeholder={t('input.targetPlaceholder')}
              size="sm"
              aria-label={t('input.target')}
              spellCheck={false}
            />
          </label>
          <div className="flex gap-2">
            <label className="block min-w-0 flex-1 space-y-1 text-xs text-fg-muted">
              <span>{t('input.method')}</span>
              <Input value={method} onChange={(event) => setMethod(event.target.value)} size="sm" aria-label={t('input.method')} spellCheck={false} />
            </label>
            <label className="block min-w-0 flex-1 space-y-1 text-xs text-fg-muted">
              <span>{t('input.credentials')}</span>
              <Select
                value={credentials}
                onValueChange={setCredentials}
                size="sm"
                aria-label={t('input.credentials')}
                options={[
                  { value: 'omit', label: t('input.credentials.omit') },
                  { value: 'same-origin', label: t('input.credentials.sameOrigin') },
                  { value: 'include', label: t('input.credentials.include') },
                ]}
              />
            </label>
          </div>
          <span className="block text-xs text-fg-muted">{t('input.headers')}</span>
          <HeaderRows items={headers} onChange={setHeaders} removeLabel={t('input.remove')} />
        </Panel>

        <ResponseFields
          title={t('input.preflight')}
          icon={<CornerUpRight />}
          block={preflight}
          onChange={setPreflight}
        />
        <ResponseFields
          title={t('input.actual')}
          icon={<CornerDownLeft />}
          block={actual}
          onChange={setActual}
        />
      </div>

      <Panel
        icon={<ShieldCheck />}
        title={t('result.title')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        extra={
          report && (
            <Badge variant={OVERALL_VARIANT[report.overall]}>{t(`status.${report.overall}`)}</Badge>
          )
        }
        footer={
          report && (
            <div className="flex w-full items-center gap-2">
              <Button size="sm" variant="soft" onClick={() => void exportReport('markdown', false)}>
                {t('export.markdown')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('json', false)}>
                {t('export.json')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="sm" onClick={() => void exportReport('markdown', true)}>
                  {t('export.saveMd')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('json', true)}>
                  {t('export.saveJson')}
                </Button>
              </span>
            </div>
          )
        }
      >
        {failure && (
          <p className="flex items-center gap-1.5 text-xs text-danger" role="alert">
            {errorMessage(failure)}
          </p>
        )}
        {!report && !failure && (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<ShieldCheck />} title={t('result.empty')} description={t('input.empty')} />
          </div>
        )}
        {report && request && (
          <>
            <p className="text-[13px] font-medium text-fg" role="status">
              {outcome}
            </p>

            <section className="space-y-1.5 rounded-control border border-border bg-surface-2 p-3">
              <h3 className="text-xs font-semibold text-fg">{t('result.request')}</h3>
              <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
                <span>{t('result.origin')}</span>
                <code className="font-mono text-fg" data-selectable>{request.origin}</code>
              </p>
              <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
                <span>{t('result.target')}</span>
                <code className="min-w-0 truncate font-mono text-fg" data-selectable>{request.target}</code>
              </p>
              <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
                <span>{t('result.methodLine')}</span>
                <code className="font-mono text-fg">{request.method}</code>
                <code className="font-mono text-fg-subtle">{request.credentials}</code>
              </p>
              <p className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
                <span>{t('result.preflightNeeded')}</span>
                <Badge variant={request.preflightNeeded ? 'warning' : 'neutral'}>
                  {request.preflightNeeded ? t('result.yes') : t('result.no')}
                </Badge>
              </p>
              <p className="flex flex-wrap items-center gap-1.5 text-xs text-fg-muted">
                <span>{t('result.unsafeHeaders')}</span>
                {request.unsafeHeaderNames.length === 0 ? (
                  <span className="text-fg-subtle">{t('result.none')}</span>
                ) : (
                  request.unsafeHeaderNames.map((name) => (
                    <code key={name} className="rounded-control bg-active px-1.5 py-0.5 font-mono text-fg">
                      {name}
                    </code>
                  ))
                )}
              </p>
            </section>

            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.options')}</h3>
              <p className="text-[11px] text-fg-subtle">{t('result.optionsHint')}</p>
              <div className="flex items-start gap-2">
                <pre className="min-w-0 flex-1 overflow-x-auto rounded-control bg-active px-3 py-2 font-mono text-xs text-fg" data-selectable>
                  {`OPTIONS ${request.options.url}\n${request.options.headers
                    .map((header) => `${header.name}: ${header.value}`)
                    .join('\n')}`}
                </pre>
                <CopyButton text={`OPTIONS ${request.options.url}\n${request.options.headers.map((header) => `${header.name}: ${header.value}`).join('\n')}`} />
              </div>
            </section>

            {report.preflight.status !== 'skipped' && (
              <section className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <h3 className="text-xs font-semibold text-fg">{t('result.stage.preflight')}</h3>
                  <Badge variant={STAGE_VARIANT[report.preflight.status]}>{t(`status.${report.preflight.status}`)}</Badge>
                </div>
                <ul className="space-y-1.5">
                  {report.preflight.conditions.map((condition) => (
                    <ConditionRow key={condition.id} condition={condition} />
                  ))}
                </ul>
                {report.preflight.reasons.map((reason, index) => (
                  <p key={index} className="text-xs text-warning">
                    {errorMessage(reason)}
                  </p>
                ))}
              </section>
            )}

            <section className="space-y-1.5">
              <div className="flex items-center gap-2">
                <h3 className="text-xs font-semibold text-fg">{t('result.stage.reading')}</h3>
                <Badge variant={STAGE_VARIANT[report.reading.status]}>{t(`status.${report.reading.status}`)}</Badge>
              </div>
              <ul className="space-y-1.5">
                {report.reading.conditions.map((condition) => (
                  <ConditionRow key={condition.id} condition={condition} />
                ))}
              </ul>
              {report.reading.reasons.map((reason, index) => (
                <p key={index} className="text-xs text-fg-muted">
                  {errorMessage(reason)}
                </p>
              ))}
              {report.reading.readable.length > 0 && (
                <p className="flex flex-wrap items-center gap-1.5 text-xs text-fg-muted">
                  <span>{t('result.readable')}</span>
                  {report.reading.readable.map((name) => (
                    <code key={name} className="rounded-control bg-active px-1.5 py-0.5 font-mono text-success">
                      {name}
                    </code>
                  ))}
                </p>
              )}
              {report.reading.suppressed.map((diag, index) => (
                <p key={index} className="text-xs text-warning">
                  {errorMessage(diag)}
                </p>
              ))}
            </section>

            {report.diagnostics.length > 0 && (
              <section className="space-y-1.5">
                <h3 className="text-xs font-semibold text-fg">{t('result.diagnostics')}</h3>
                {report.diagnostics.map((diag, index) => (
                  <p key={index} className="flex items-start gap-1.5 text-xs text-warning">
                    {errorMessage(diag)}
                  </p>
                ))}
              </section>
            )}
          </>
        )}
      </Panel>
    </div>
  )
}
