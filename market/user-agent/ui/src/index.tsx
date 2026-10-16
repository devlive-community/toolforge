import { useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Select, Spinner, Tabs, Tooltip, toast } from '@toolforge/ui'
import { host, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Bot as BotIcon, ChevronDown, Download, FileText, FolderOpen, Laptop, ListChecks, ScanSearch, Sparkles } from 'lucide-react'
import { SAMPLE_LOG, SAMPLE_UA, type Bucket, type Hit, type Mode, type Output, type Parsed, type Part } from './types'

type Tab = 'single' | 'batch'

/** 表格中最多渲染的行数；导出包含全部 */
const MAX_VISIBLE = 1000

function RuleHit({ hit }: { hit: Hit }) {
  const { t } = usePlugin()
  return (
    <Tooltip content={t('single.pattern', { pattern: hit.pattern })}>
      <p className="mt-1 flex min-w-0 flex-wrap items-center gap-1.5 text-[11px] text-fg-subtle">
        <code className="rounded bg-surface-2 px-1 font-mono">{hit.rule}</code>
        <code className="truncate font-mono text-fg-muted" data-selectable>
          {hit.matched}
        </code>
      </p>
    </Tooltip>
  )
}

function PartCard({ label, part }: { label: string; part: Part | null }) {
  const { t } = usePlugin()
  return (
    <div className="min-w-0 rounded-control border border-border p-3">
      <p className="text-[11px] text-fg-subtle">{label}</p>
      {part ? (
        <>
          <p className="mt-0.5 truncate text-sm font-semibold text-fg" data-selectable>
            {part.name} {part.version && <span className="font-normal text-fg-muted">{part.version}</span>}
          </p>
          <RuleHit hit={part.hit} />
        </>
      ) : (
        <p className="mt-0.5 text-sm text-fg-subtle">{t('single.unknown')}</p>
      )}
    </div>
  )
}

function ParsedView({ parsed }: { parsed: Parsed }) {
  const { t } = usePlugin()
  const device = parsed.device
  return (
    <div className="space-y-3">
      {parsed.bot && (
        <div className="flex items-start gap-2 rounded-control bg-warning-soft px-3 py-2 text-[13px] text-warning">
          <BotIcon className="mt-0.5 size-4 shrink-0" />
          <div className="min-w-0">
            <p className="font-medium">
              {t('single.botFound', { name: parsed.bot.name, category: t(`botCategory.${parsed.bot.category}`) })}
              {parsed.bot.version && ` ${parsed.bot.version}`}
            </p>
            <RuleHit hit={parsed.bot.hit} />
          </div>
        </div>
      )}
      <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2">
        <PartCard label={t('field.browser')} part={parsed.browser} />
        <PartCard label={t('field.engine')} part={parsed.engine} />
        <PartCard label={t('field.os')} part={parsed.os} />
        <div className="min-w-0 rounded-control border border-border p-3">
          <p className="text-[11px] text-fg-subtle">{t('field.device')}</p>
          <p className="mt-0.5 truncate text-sm font-semibold text-fg" data-selectable>
            {t(`device.${device.kind}`)}
            {(device.vendor || device.model) && <span className="font-normal text-fg-muted"> · {[device.vendor, device.model].filter(Boolean).join(' ')}</span>}
          </p>
          {device.hit && <RuleHit hit={device.hit} />}
        </div>
      </div>
      {parsed.notes.length > 0 && (
        <ul className="space-y-1 rounded-control bg-surface-2 px-3 py-2 text-xs text-fg-muted">
          {parsed.notes.map((note) => (
            <li key={note}>{t(`notes.${note}`)}</li>
          ))}
        </ul>
      )}
    </div>
  )
}

function Single() {
  const { t, errorMessage } = usePlugin()
  const [ua, setUa] = useState(SAMPLE_UA)
  useLaunchInput((text) => setUa(text.trim()))
  const has = ua.trim().length > 0
  const { result, error, pending } = useDebouncedCall<Parsed>(has ? 'parse' : null, has ? { ua } : null, [ua])

  return (
    <Panel
      icon={<ScanSearch />}
      title={t('single.title')}
      extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
      actions={
        <Button size="sm" onClick={() => setUa(navigator.userAgent)}>
          <Laptop />
          {t('single.current')}
        </Button>
      }
      className="h-full"
      bodyClassName="space-y-3 overflow-auto p-3"
    >
      <Input className="w-full font-mono text-[13px]" value={ua} onChange={(event) => setUa(event.target.value)} placeholder={t('single.placeholder')} aria-label={t('single.title')} />
      {error ? (
        <p className="text-[13px] text-danger">{errorMessage(error)}</p>
      ) : result && has ? (
        <ParsedView parsed={result} />
      ) : (
        <Empty icon={<ScanSearch />} title={t('single.empty')} description={t('single.hint')} />
      )}
    </Panel>
  )
}

function Distribution({ title, buckets, total, translate }: { title: string; buckets: Bucket[]; total: number; translate?: (name: string) => string }) {
  if (buckets.length === 0) return null
  return (
    <div className="min-w-0 space-y-1">
      <p className="text-[11px] font-medium text-fg-subtle">{title}</p>
      {buckets.slice(0, 8).map((bucket) => {
        const percent = total > 0 ? (bucket.count / total) * 100 : 0
        return (
          <div key={bucket.name} className="text-xs">
            <div className="flex justify-between gap-2">
              <span className="truncate text-fg">{translate ? translate(bucket.name) : bucket.name}</span>
              <span className="shrink-0 text-fg-muted tabular-nums">
                {bucket.count} · {percent.toFixed(1)}%
              </span>
            </div>
            <div className="mt-0.5 h-1 rounded-full bg-surface-2">
              <div className="h-1 rounded-full bg-primary" style={{ width: `${Math.max(percent, 1)}%` }} />
            </div>
          </div>
        )
      })}
    </div>
  )
}

const label = (part: Part | null) => (part ? [part.name, part.version].filter(Boolean).join(' ') : '—')

function Batch() {
  const { t, call, errorMessage } = usePlugin()
  const [input, setInput] = useState(SAMPLE_LOG)
  const [mode, setMode] = useState<Mode>('log')
  const [column, setColumn] = useState('')
  const has = input.trim().length > 0
  const args = { input, mode, column }
  const { result, error, pending } = useDebouncedCall<Output>(has ? 'analyze' : null, has ? args : null, [input, mode, column])

  const openFile = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('batch.filter'), extensions: ['log', 'txt', 'csv', 'tsv'] }])
      if (!path) return
      const text = await host.fs.readText(path)
      setInput(text)
      const lower = path.toLowerCase()
      setMode(lower.endsWith('.csv') || lower.endsWith('.tsv') ? 'csv' : lower.endsWith('.log') ? 'log' : 'lines')
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const exportReport = async (format: 'csv' | 'json', target: 'copy' | 'save') => {
    try {
      const text = await call<string>('export', { ...args, format })
      if (target === 'copy') {
        await host.clipboard.writeText(text)
        toast.success(t('common:copied'))
        return
      }
      const path = await host.dialog.saveFile(`user-agents.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const stats = result?.stats
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel
        icon={<FileText />}
        title={t('batch.input')}
        actions={
          <>
            <SegmentedControl<Mode>
              size="sm"
              value={mode}
              onValueChange={setMode}
              aria-label={t('batch.mode')}
              options={[
                { value: 'log', label: t('batch.log') },
                { value: 'lines', label: t('batch.lines') },
                { value: 'csv', label: t('batch.csv') },
              ]}
            />
            <Button size="icon-sm" variant="ghost" aria-label={t('batch.open')} onClick={() => void openFile()}>
              <FolderOpen />
            </Button>
          </>
        }
        footer={
          mode === 'csv' ? (
            <span className="flex items-center gap-2">
              {t('batch.column')}
              {result && result.columns.length > 0 ? (
                <Select<string>
                  size="sm"
                  value={column || null}
                  placeholder={t('batch.autoColumn')}
                  onValueChange={setColumn}
                  aria-label={t('batch.column')}
                  options={result.columns.map((name) => ({ value: name, label: name }))}
                />
              ) : (
                <Input size="sm" value={column} onChange={(event) => setColumn(event.target.value)} placeholder={t('batch.autoColumn')} aria-label={t('batch.column')} />
              )}
            </span>
          ) : (
            <span>{t(`batch.${mode}Hint`)}</span>
          )
        }
      >
        <CodeEditor value={input} onChange={setInput} language="text" placeholder={t('batch.placeholder')} aria-label={t('batch.input')} />
      </Panel>

      <Panel
        icon={<ListChecks />}
        title={t('batch.result')}
        extra={
          pending ? (
            <Spinner className="size-3.5 text-fg-subtle" />
          ) : (
            stats && (
              <span className="flex flex-wrap items-center gap-1.5">
                <Badge>{t('batch.total', { count: stats.total })}</Badge>
                <Badge>{t('batch.unique', { count: stats.unique })}</Badge>
                {stats.bots > 0 && <Badge variant="warning">{t('batch.bots', { count: stats.bots })}</Badge>}
                {result.skipped > 0 && <Badge variant="danger">{t('batch.skipped', { count: result.skipped })}</Badge>}
              </span>
            )
          )
        }
        actions={
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={!result || result.rows.length === 0}>
                <Download />
                {t('export.title')}
                <ChevronDown />
              </Button>
            }
            items={[
              { key: 'copy-csv', label: t('export.copyCsv'), onSelect: () => void exportReport('csv', 'copy') },
              { key: 'save-csv', label: t('export.saveCsv'), onSelect: () => void exportReport('csv', 'save') },
              { key: 'copy-json', label: t('export.copyJson'), onSelect: () => void exportReport('json', 'copy') },
              { key: 'save-json', label: t('export.saveJson'), onSelect: () => void exportReport('json', 'save') },
            ]}
          />
        }
        footer={
          result && (
            <span className="flex items-center gap-1.5">
              <Sparkles className="size-3.5" />
              {t('batch.rules', { version: result.rulesVersion })}
            </span>
          )
        }
        bodyClassName="space-y-4 overflow-auto p-3"
      >
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result || !stats || result.rows.length === 0 ? (
          <Empty icon={<ListChecks />} title={t('batch.empty')} description={t('batch.emptyHint')} />
        ) : (
          <>
            <div className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-4">
              <Distribution title={t('field.device')} buckets={stats.devices} total={stats.total} translate={(name) => t(`device.${name}`)} />
              <Distribution title={t('field.browser')} buckets={stats.browsers} total={stats.total - stats.bots} />
              <Distribution title={t('field.os')} buckets={stats.systems} total={stats.total - stats.bots} />
              <Distribution title={t('batch.botNames')} buckets={stats.botNames} total={stats.bots} />
              <Distribution title={t('batch.botCategories')} buckets={stats.botCategories} total={stats.bots} translate={(name) => t(`botCategory.${name}`)} />
            </div>
            {result.truncated && <p className="text-xs text-warning">{t('batch.truncated', { count: result.rows.length })}</p>}
            <div className="overflow-auto rounded-control border border-border">
              <table className="w-full border-collapse text-xs">
                <thead className="sticky top-0 bg-surface-2 text-fg-muted">
                  <tr>
                    <th className="px-2 py-1.5 text-right font-medium">{t('batch.count')}</th>
                    <th className="px-2 py-1.5 text-left font-medium">{t('field.ua')}</th>
                    <th className="px-2 py-1.5 text-left font-medium">{t('field.browser')}</th>
                    <th className="px-2 py-1.5 text-left font-medium">{t('field.os')}</th>
                    <th className="px-2 py-1.5 text-left font-medium">{t('field.device')}</th>
                  </tr>
                </thead>
                <tbody>
                  {result.rows.slice(0, MAX_VISIBLE).map((row) => (
                    <tr key={row.ua} className="border-t border-border align-top">
                      <td className="px-2 py-1.5 text-right text-fg-muted tabular-nums">{row.count}</td>
                      <td className="max-w-80 px-2 py-1.5">
                        <p className="truncate font-mono text-fg" title={row.ua} data-selectable>
                          {row.ua}
                        </p>
                      </td>
                      <td className="px-2 py-1.5 text-fg">{row.bot ? <Badge variant="warning">{row.bot.name}</Badge> : label(row.browser)}</td>
                      <td className="px-2 py-1.5 text-fg">{label(row.os)}</td>
                      <td className="px-2 py-1.5 text-fg-muted">{t(`device.${row.device.kind}`)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {result.rows.length > MAX_VISIBLE && <p className="text-xs text-fg-subtle">{t('batch.moreRows', { count: MAX_VISIBLE })}</p>}
          </>
        )}
      </Panel>
    </div>
  )
}

function UserAgentTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('single')
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'single', label: t('tabs.single'), icon: <ScanSearch /> },
          { value: 'batch', label: t('tabs.batch'), icon: <ListChecks /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        <div className="h-full" hidden={tab !== 'single'}>
          <Single />
        </div>
        <div className="h-full" hidden={tab !== 'batch'}>
          <Batch />
        </div>
      </div>
    </div>
  )
}

export default UserAgentTool
