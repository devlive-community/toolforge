import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, Panel, Spinner, Tooltip, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CircleX, Code2, FileJson, FileText, Globe, RefreshCw, TriangleAlert } from 'lucide-react'
import type { Report, Source, SourceStatusKind } from './types'

type Translate = (key: string, params?: Record<string, unknown>) => string

const STATUS_BADGE: Record<SourceStatusKind, 'success' | 'warning' | 'danger' | 'info' | undefined> = {
  configured: 'success',
  disabled: 'warning',
  notConfigured: undefined,
  failed: 'danger',
  unsupported: undefined,
}

const collectedAt = new Intl.DateTimeFormat(undefined, { dateStyle: 'short', timeStyle: 'medium' })

/** Markdown 摘要：来源卡 + 环境变量列表（值已在后端脱敏） */
function toMarkdown(report: Report, time: Date, t: Translate): string {
  const lines: string[] = [
    `# ${t('export.title')}`,
    '',
    `- ${t('export.collected')}: ${collectedAt.format(time)}`,
    '',
  ]
  for (const source of report.sources) {
    lines.push(`## ${t(`sources.${source.id}`)} (${t(`scope.${source.scope}`)})`)
    lines.push(`- ${t('labels.status')}: ${t(`status.${source.status.kind}`)}`)
    for (const entry of source.entries) {
      const warning = entry.warnings.map((code) => t(`warnings.${code}`)).join(', ')
      lines.push(`- ${entry.protocol.toUpperCase()}: \`${entry.host}:${entry.port}\`${warning ? ` (${warning})` : ''}`)
    }
    if (source.pacUrl) lines.push(`- PAC: ${source.pacUrl}`)
    if (source.autoDiscover !== null) {
      lines.push(`- ${t('labels.autoDiscover')}: ${source.autoDiscover ? t('common:yes') : t('common:no')}`)
    }
    if (source.exclusions.length > 0) lines.push(`- ${t('labels.exclusions')}: ${source.exclusions.join(', ')}`)
    lines.push('')
  }
  lines.push(`## ${t('sections.env')}`)
  for (const env of report.envVars) {
    lines.push(`- \`${env.name}\`: ${env.value}`)
  }
  return lines.join('\n')
}

export function ProxyInspector() {
  const { t, call, errorMessage } = usePlugin()
  const [report, setReport] = useState<Report | null>(null)
  const [loading, setLoading] = useState(true)
  const [collected, setCollected] = useState<Date | null>(null)
  const [showRaw, setShowRaw] = useState<string | null>(null)

  const refresh = async () => {
    setLoading(true)
    try {
      const next = await call<Report>('inspect', {})
      setReport(next)
      setCollected(new Date())
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setLoading(false)
    }
  }

  // 挂载时立即采集一次
  const refreshRef = useRef(refresh)
  useEffect(() => {
    refreshRef.current = refresh
  })
  useEffect(() => {
    void refreshRef.current()
  }, [])

  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const save = (filename: string, text: string) =>
    run(async () => {
      const path = await host.dialog.saveFile(filename)
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('export.saved', { name: filename }))
    })

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-1.5">
        <Button size="sm" onClick={refresh} disabled={loading}>
          {loading ? <Spinner /> : <RefreshCw />}
          {t('refresh')}
        </Button>
        <span className="text-xs text-fg-muted tabular-nums">
          {collected && t('collectedAt', { time: collectedAt.format(collected) })}
        </span>
        <div className="ml-auto flex items-center gap-1.5">
          <Tooltip content={t('export.json')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('export.json')}
              disabled={!report}
              onClick={() => {
                if (report) {
                  void save(
                    'proxy-report.json',
                    JSON.stringify({ collectedAt: collected?.toISOString(), ...report }, null, 2),
                  )
                }
              }}
            >
              <FileJson />
            </Button>
          </Tooltip>
          <Tooltip content={t('export.markdown')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('export.markdown')}
              disabled={!report || !collected}
              onClick={() => {
                if (report && collected) void save('proxy-report.md', toMarkdown(report, collected, t))
              }}
            >
              <FileText />
            </Button>
          </Tooltip>
        </div>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {!loading && !report ? (
          <Panel>
            <Empty icon={<CircleX />} title={t('load.failed')} />
          </Panel>
        ) : (
          <>
            <SectionTitle icon={<Globe />} title={t('sections.sources')} count={report?.sources.length ?? 0} />
            {loading && !report && (
              <Panel>
                <div className="flex items-center justify-center gap-2 py-8 text-fg-muted">
                  <Spinner className="size-5" />
                  <span className="text-xs">{t('loading')}</span>
                </div>
              </Panel>
            )}
            {report?.sources.map((source) => (
              <SourceCard key={source.id} source={source} showRaw={showRaw} setShowRaw={setShowRaw} />
            ))}

            <SectionTitle icon={<Code2 />} title={t('sections.env')} count={report?.envVars.length ?? 0} />
            <Panel title={t('sections.env')} bodyClassName="space-y-2 p-3">
              <p className="text-xs text-fg-subtle">{t('env.hint')}</p>
              {!report || report.envVars.length === 0 ? (
                <p className="py-1 text-[13px] text-fg-muted">{t('env.empty')}</p>
              ) : (
                <ul className="space-y-1">
                  {report.envVars.map((env) => (
                    <li
                      key={env.name}
                      className="flex flex-wrap items-baseline gap-2 rounded-control bg-surface-2 px-2.5 py-1.5"
                    >
                      <span className="font-mono text-[12.5px] text-fg">{env.name}</span>
                      <span className="min-w-0 flex-1 break-all font-mono text-[12.5px] text-fg-muted" data-selectable>
                        {env.value}
                      </span>
                      {env.isExclusions && <Badge>{t('env.exclusions')}</Badge>}
                    </li>
                  ))}
                </ul>
              )}
            </Panel>
          </>
        )}
      </div>
    </div>
  )
}

function SectionTitle({ icon, title, count }: { icon: React.ReactNode; title: string; count: number }) {
  return (
    <div className="flex items-center gap-1.5 px-1">
      {icon}
      <span className="text-[13px] font-medium text-fg">{title}</span>
      {count > 0 && <Badge>{count}</Badge>}
    </div>
  )
}

function SourceCard({
  source,
  showRaw,
  setShowRaw,
}: {
  source: Source
  showRaw: string | null
  setShowRaw: (id: string | null) => void
}) {
  const { t, errorMessage } = usePlugin()
  const badge = STATUS_BADGE[source.status.kind]
  return (
    <Panel
      icon={source.status.kind === 'failed' ? <TriangleAlert /> : <Globe />}
      title={t(`sources.${source.id}`)}
      extra={
        <div className="flex items-center gap-1.5">
          <Badge>{t(`scope.${source.scope}`)}</Badge>
          {badge && <Badge variant={badge}>{t(`status.${source.status.kind}`)}</Badge>}
          {!badge && <Badge>{t(`status.${source.status.kind}`)}</Badge>}
        </div>
      }
      bodyClassName="space-y-2.5 p-3"
    >
      {source.status.kind === 'failed' && source.status.code && (
        <p className="text-[13px] text-danger">{errorMessage({ code: source.status.code })}</p>
      )}
      {source.status.kind === 'unsupported' && (
        <p className="text-[13px] text-fg-muted">{t('statusHint.unsupported')}</p>
      )}
      {source.status.kind === 'disabled' && <p className="text-[13px] text-fg-muted">{t('statusHint.disabled')}</p>}
      {source.status.kind === 'notConfigured' && (
        <p className="text-[13px] text-fg-muted">{t('statusHint.notConfigured')}</p>
      )}

      {source.entries.length > 0 && (
        <ul className="space-y-1">
          {source.entries.map((entry) => (
            <li
              key={`${entry.protocol}-${entry.host}-${entry.port}`}
              className="flex flex-wrap items-baseline gap-2 rounded-control bg-surface-2 px-2.5 py-1.5"
            >
              <Badge variant="info">{entry.protocol.toUpperCase()}</Badge>
              <span className="font-mono text-[12.5px] text-fg" data-selectable>
                {entry.host}:{entry.port}
              </span>
              {entry.warnings.map((warning) => (
                <span key={warning} className="text-[11.5px] text-warning">
                  {t(`warnings.${warning}`)}
                </span>
              ))}
            </li>
          ))}
        </ul>
      )}

      {source.pacUrl && (
        <p className="break-all text-[12.5px] text-fg-muted">
          <span className="text-fg-subtle">{t('labels.pac')}: </span>
          <span className="font-mono" data-selectable>
            {source.pacUrl}
          </span>
        </p>
      )}
      {source.autoDiscover !== null && (
        <p className="text-[12.5px] text-fg-muted">
          <span className="text-fg-subtle">{t('labels.autoDiscover')}: </span>
          {t(source.autoDiscover ? 'common:yes' : 'common:no')}
        </p>
      )}
      {source.exclusions.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {source.exclusions.map((item) => (
            <Badge key={item}>{item}</Badge>
          ))}
        </div>
      )}

      {source.raw && (
        <div>
          <button
            type="button"
            onClick={() => setShowRaw(showRaw === source.id ? null : source.id)}
            className="text-[12.5px] text-primary outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring"
          >
            {showRaw === source.id ? t('raw.hide') : t('raw.show')}
          </button>
          {showRaw === source.id && (
            <pre
              className="mt-1.5 max-h-56 overflow-auto rounded-control bg-surface-2 p-2.5 font-mono text-[11.5px] leading-normal text-fg-muted"
              data-selectable
            >
              {JSON.stringify(source.raw, null, 2)}
            </pre>
          )}
        </div>
      )}
    </Panel>
  )
}
