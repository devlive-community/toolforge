import { useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Select, Spinner, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, Download, FileText, FolderOpen, ListChecks, ShieldCheck } from 'lucide-react'
import { SAMPLE, type Mode, type Output, type Problem, type Row, type Status } from './types'

const STATUS_VARIANT: Record<Status, 'success' | 'warning' | 'danger'> = { valid: 'success', unconfirmed: 'warning', invalid: 'danger' }

function ProblemText({ problem }: { problem: Problem }) {
  const { t } = usePlugin()
  switch (problem.kind) {
    case 'length':
      return <>{t('problems.length', { found: problem.found })}</>
    case 'char':
      return <>{t('problems.char', { position: problem.position, found: problem.found })}</>
    case 'check':
      return <>{t('problems.check', { expected: problem.expected, found: problem.found })}</>
  }
}

function RowCard({ row }: { row: Row }) {
  const { t } = usePlugin()
  const changes = (['trimmed', 'removedSpaces', 'uppercased', 'fullwidth'] as const).filter((key) => row.normalized[key])
  const fields = row.fields
  return (
    <li className="space-y-1.5 rounded-control border border-border p-2.5">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-[11px] text-fg-subtle tabular-nums">{t('row.line', { line: row.line })}</span>
        <code className="font-mono text-[13px] text-fg" data-selectable>
          {row.normalized.value}
        </code>
        <Badge variant={STATUS_VARIANT[row.status]}>{t(`status.${row.status}`)}</Badge>
        {changes.length > 0 && (
          <span className="text-[11px] text-warning">{t('row.normalized', { changes: changes.map((c) => t(`changes.${c}`)).join(t('row.separator')) })}</span>
        )}
      </div>
      {row.problems.length > 0 && (
        <ul className="space-y-0.5 text-xs text-danger">
          {row.problems.map((problem, index) => (
            <li key={index}>
              <ProblemText problem={problem} />
            </li>
          ))}
        </ul>
      )}
      {fields && (
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-0.5 text-xs">
          <dt className="text-fg-subtle">{t('fields.department')}</dt>
          <dd className="text-fg">
            <code className="font-mono">{fields.department.code}</code> {fields.department.name ? t(`departments.${fields.department.name}`) : t('fields.unknown')}
          </dd>
          <dt className="text-fg-subtle">{t('fields.category')}</dt>
          <dd className="text-fg">
            <code className="font-mono">{fields.category.code}</code>{' '}
            {fields.category.name ? t(`categories.${fields.department.name}.${fields.category.name}`, { defaultValue: t(`categories.common.${fields.category.name}`) }) : t('fields.unknown')}
          </dd>
          <dt className="text-fg-subtle">{t('fields.region')}</dt>
          <dd className="text-fg">
            <code className="font-mono">{fields.region.code}</code> {fields.region.name ? t('fields.province', { name: t(`provinces.${fields.region.name}`) }) : t('fields.regionUnknown')}
          </dd>
          <dt className="text-fg-subtle">{t('fields.subject')}</dt>
          <dd className="text-fg">
            <code className="font-mono">{fields.subject}</code>
            {!fields.orgCheckOk && <span className="ml-2 text-warning">{t('fields.orgCheck', { expected: fields.orgCheckExpected ?? '?' })}</span>}
          </dd>
          <dt className="text-fg-subtle">{t('fields.check')}</dt>
          <dd className="font-mono text-fg">{fields.check}</dd>
        </dl>
      )}
      {row.raw !== row.normalized.value && (
        <p className="text-[11px] text-fg-subtle">
          {t('row.raw')} <code className="font-mono whitespace-pre text-fg-muted">{JSON.stringify(row.raw)}</code>
        </p>
      )}
    </li>
  )
}

function UsccTool() {
  const { t, call, errorMessage } = usePlugin()
  const [input, setInput] = useState(SAMPLE)
  const [mode, setMode] = useState<Mode>('lines')
  const [column, setColumn] = useState('')
  useLaunchInput((text) => {
    setInput(text)
    setMode('lines')
  })

  const has = input.trim().length > 0
  const args = { input, mode, column }
  const { result, error, pending } = useDebouncedCall<Output>(has ? 'validate' : null, has ? args : null, [input, mode, column])

  const openFile = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('input.filter'), extensions: ['csv', 'txt', 'tsv'] }])
      if (!path) return
      const text = await host.fs.readText(path)
      setInput(text)
      setMode(path.toLowerCase().endsWith('.csv') ? 'csv' : 'lines')
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
      const path = await host.dialog.saveFile(`uscc-report.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel
        icon={<FileText />}
        title={t('input.title')}
        actions={
          <>
            <SegmentedControl<Mode>
              size="sm"
              value={mode}
              onValueChange={setMode}
              aria-label={t('input.mode')}
              options={[
                { value: 'lines', label: t('input.lines') },
                { value: 'csv', label: t('input.csv') },
              ]}
            />
            <Button size="icon-sm" variant="ghost" aria-label={t('input.open')} onClick={() => void openFile()}>
              <FolderOpen />
            </Button>
          </>
        }
        footer={
          mode === 'csv' && (
            <span className="flex items-center gap-2">
              {t('input.column')}
              {result && result.columns.length > 0 ? (
                <Select<string>
                  size="sm"
                  value={column || result.columns[0]}
                  onValueChange={setColumn}
                  aria-label={t('input.column')}
                  options={result.columns.map((name) => ({ value: name, label: name }))}
                />
              ) : (
                <Input size="sm" value={column} onChange={(event) => setColumn(event.target.value)} placeholder={t('input.columnPlaceholder')} aria-label={t('input.column')} />
              )}
            </span>
          )
        }
      >
        <CodeEditor value={input} onChange={setInput} language="text" placeholder={t('input.placeholder')} aria-label={t('input.title')} />
      </Panel>

      <Panel
        icon={<ListChecks />}
        title={t('result.title')}
        extra={
          pending ? (
            <Spinner className="size-3.5 text-fg-subtle" />
          ) : (
            result && (
              <span className="flex flex-wrap items-center gap-1.5">
                <Badge variant="success">{t('summary.valid', { count: result.summary.valid })}</Badge>
                {result.summary.unconfirmed > 0 && <Badge variant="warning">{t('summary.unconfirmed', { count: result.summary.unconfirmed })}</Badge>}
                {result.summary.invalid > 0 && <Badge variant="danger">{t('summary.invalid', { count: result.summary.invalid })}</Badge>}
                {result.summary.empty > 0 && <Badge>{t('summary.empty', { count: result.summary.empty })}</Badge>}
              </span>
            )
          )
        }
        actions={
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={!result}>
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
        footer={result && <span className="flex items-center gap-1.5"><ShieldCheck className="size-3.5" />{t('result.rules', { rules: result.rules })}</span>}
        bodyClassName="overflow-auto p-3"
      >
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result || result.rows.length === 0 ? (
          <Empty icon={<ListChecks />} title={t('result.empty')} description={t('result.disclaimer')} />
        ) : (
          <div className="space-y-3">
            <p className={cn('rounded-control bg-surface-2 px-3 py-2 text-xs text-fg-muted')}>{t('result.disclaimer')}</p>
            <ul className="space-y-2">
              {result.rows.map((row) => (
                <RowCard key={`${row.line}-${row.raw}`} row={row} />
              ))}
            </ul>
          </div>
        )}
      </Panel>
    </div>
  )
}

export default UsccTool
