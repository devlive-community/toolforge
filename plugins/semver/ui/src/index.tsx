import { useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Spinner, Switch, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, Download, FileText, ListOrdered, TriangleAlert } from 'lucide-react'
import { PRESETS, SAMPLE_VERSIONS, type Analysis, type Order, type Row } from './types'

function Result({ row }: { row: Row }) {
  const { t, errorMessage } = usePlugin()
  if (row.error) return <span className="text-danger">{errorMessage(row.error)}</span>
  if (!row.outcome) return null
  if (row.outcome.kind === 'match') return <Badge variant="success">{t('result.match')}</Badge>
  return (
    <span className="flex flex-wrap items-center gap-1.5">
      <Badge variant="danger">{t('result.noMatch')}</Badge>
      <span className="text-fg-muted">
        {row.outcome.kind === 'prerelease' ? t('result.prerelease') : t('result.failed', { comparator: row.outcome.comparator })}
      </span>
    </span>
  )
}

function SemverTool() {
  const { t, errorMessage } = usePlugin()
  const [versions, setVersions] = useState(SAMPLE_VERSIONS)
  const [range, setRange] = useState('^0.2.3')
  const [includePrerelease, setIncludePrerelease] = useState(false)
  const [order, setOrder] = useState<Order>('asc')

  const { result, error, pending } = useDebouncedCall<Analysis>('analyze', { versions, range, includePrerelease, order }, [versions, range, includePrerelease, order])
  const equal = new Set(result?.equalPrecedence.flat() ?? [])

  const saveCsv = async () => {
    if (!result) return
    try {
      const path = await host.dialog.saveFile('semver.csv', [{ name: 'CSV', extensions: ['csv'] }])
      if (!path) return
      await host.fs.writeText(path, result.csv)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="shrink-0 space-y-2 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            value={range}
            onChange={(event) => setRange(event.target.value)}
            placeholder={t('range.placeholder')}
            className="font-mono text-[14px]"
            wrapperClassName="min-w-60 flex-1"
            size="lg"
            invalid={!!result?.rangeError}
            aria-label={t('range.label')}
            spellCheck={false}
          />
          <DropdownMenu
            trigger={
              <Button>
                {t('range.presets')}
                <ChevronDown />
              </Button>
            }
            items={PRESETS.map((preset) => ({ key: preset, label: preset, onSelect: () => setRange(preset) }))}
          />
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={includePrerelease} onCheckedChange={setIncludePrerelease} aria-label={t('range.includePrerelease')} />
            {t('range.includePrerelease')}
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <Badge variant="info">{t('range.dialect')}</Badge>
          {result?.rangeError ? (
            <span className="flex items-center gap-1.5 text-danger">
              <TriangleAlert className="size-3.5" />
              {errorMessage(result.rangeError)}
            </span>
          ) : result && result.sets.length > 0 ? (
            <>
              <span className="text-fg-muted">{t('range.expanded')}</span>
              {result.sets.map((set, index) => (
                <span key={index} className="flex items-center gap-2">
                  {index > 0 && <span className="text-fg-subtle">||</span>}
                  <code className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-fg" data-selectable>
                    {set}
                  </code>
                </span>
              ))}
            </>
          ) : (
            <span className="text-fg-subtle">{t('range.empty')}</span>
          )}
        </div>
      </section>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(240px,0.6fr)_minmax(0,1.4fr)] gap-3">
        <Panel icon={<FileText />} title={t('versions.title')} footer={<span>{t('versions.hint')}</span>}>
          <CodeEditor value={versions} onChange={setVersions} language="text" placeholder={t('versions.placeholder')} aria-label={t('versions.title')} />
        </Panel>

        <Panel
          icon={<ListOrdered />}
          title={t('result.title')}
          extra={
            pending ? (
              <Spinner className="size-3.5 text-fg-subtle" />
            ) : (
              result && (
                <span className="flex items-center gap-1.5">
                  {result.matched !== null && <Badge variant="success">{t('result.matched', { count: result.matched, total: result.valid })}</Badge>}
                  {result.invalid > 0 && <Badge variant="danger">{t('result.invalid', { count: result.invalid })}</Badge>}
                </span>
              )
            )
          }
          actions={
            <>
              <SegmentedControl<Order>
                size="sm"
                value={order}
                onValueChange={setOrder}
                aria-label={t('result.order')}
                options={[
                  { value: 'asc', label: t('result.asc') },
                  { value: 'desc', label: t('result.desc') },
                ]}
              />
              <CopyButton text={result?.csv ?? ''} label={t('result.copyCsv')} variant="outline" disabled={!result} />
              <Button size="icon-sm" variant="ghost" aria-label={t('result.saveCsv')} disabled={!result} onClick={() => void saveCsv()}>
                <Download />
              </Button>
            </>
          }
          bodyClassName="overflow-auto"
        >
          {error ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
          ) : !result || result.rows.length === 0 ? (
            <Empty icon={<ListOrdered />} title={t('result.empty')} />
          ) : (
            <table className="w-full text-xs">
              <thead className="sticky top-0 bg-surface text-left text-fg-subtle">
                <tr>
                  <th className="px-3 py-2 font-normal">{t('table.line')}</th>
                  <th className="px-3 py-2 font-normal">{t('table.version')}</th>
                  <th className="px-3 py-2 font-normal">{t('table.parts')}</th>
                  <th className="px-3 py-2 font-normal">{t('table.result')}</th>
                </tr>
              </thead>
              <tbody>
                {result.rows.map((row) => (
                  <tr key={row.line} className={cn('border-t border-border', row.error && 'bg-danger-soft/40')}>
                    <td className="px-3 py-1.5 text-fg-subtle tabular-nums">{row.line}</td>
                    <td className="px-3 py-1.5 font-mono text-fg" data-selectable>
                      {row.normalized ?? row.input}
                      {equal.has(row.line) && (
                        <Badge className="ml-2" title={t('table.equalHint')}>
                          {t('table.equal')}
                        </Badge>
                      )}
                    </td>
                    <td className="px-3 py-1.5 font-mono text-fg-muted">
                      {row.major !== null &&
                        t('table.partsValue', {
                          core: `${row.major}.${row.minor}.${row.patch}`,
                          pre: row.prerelease.length ? row.prerelease.join('.') : '—',
                          build: row.build.length ? row.build.join('.') : '—',
                        })}
                    </td>
                    <td className="px-3 py-1.5">
                      <Result row={row} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Panel>
      </div>
    </div>
  )
}

export default SemverTool
