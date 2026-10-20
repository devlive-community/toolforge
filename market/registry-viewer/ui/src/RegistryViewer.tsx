import { useMemo, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, SegmentedControl, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileJson, FileSpreadsheet, GitCompareArrows, KeyRound, Search, TriangleAlert } from 'lucide-react'
import type { DiffKind, DiffOut, KeyNode, RegValue, ValueEntry } from './types'

type Mode = 'view' | 'diff'

const REG_EXTENSIONS = ['reg', 'txt']

function toCsv(rows: string[][]): string {
  return rows
    .map((row) => row.map((cell) => `"${cell.replace(/"/g, '""')}"`).join(','))
    .join('\r\n')
}

export function RegistryViewer() {
  const { t, errorMessage } = usePlugin()
  const [mode, setMode] = useState<Mode>('view')
  const [path, setPath] = useState<string | null>(null)
  const [pathA, setPathA] = useState<string | null>(null)
  const [pathB, setPathB] = useState<string | null>(null)
  const [filter, setFilter] = useState('')
  const [selected, setSelected] = useState<string | null>(null)
  const task = useTask<DiffOut>()

  const parsed = useDebouncedCall<{ doc: { keys: KeyNode[]; issues: string[] }; valueCount: number }>(
    mode === 'view' && path ? 'parse' : null,
    mode === 'view' && path ? { path } : null,
    [mode, path],
  )
  const doc = parsed.result?.doc

  const diffOut = task.status === 'succeeded' ? task.result : null

  /** 键过滤：按路径子串 */
  const keys = useMemo(() => {
    const list = mode === 'diff' ? diffOut?.docA.keys : doc?.keys
    if (!list) return []
    if (!filter.trim()) return list
    const needle = filter.trim().toLowerCase()
    return list.filter((key) => key.path.toLowerCase().includes(needle))
  }, [mode, doc, diffOut, filter])

  const selectedKey = keys.find((key) => key.path === selected) ?? keys[0] ?? null

  const pick = async (setter: (path: string | null) => void) => {
    try {
      const next = await host.dialog.openFile([{ name: t('file.filter'), extensions: REG_EXTENSIONS }])
      if (next) setter(next)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const runDiff = () =>
    run(async () => {
      if (!pathA || !pathB) return
      task.start('diff', { pathA, pathB })
    })

  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  /** 差异 CSV：类型/路径/值名/A/B/行号 */
  const exportDiffCsv = () => {
    if (!diffOut) return
    const rows: string[][] = [['kind', 'path', 'value', 'a', 'b', 'lineA', 'lineB']]
    for (const entry of diffOut.report.entries) {
      rows.push([
        entry.kind,
        entry.path,
        entry.valueName ?? '',
        entry.a ?? '',
        entry.b ?? '',
        entry.lineA?.toString() ?? '',
        entry.lineB?.toString() ?? '',
      ])
    }
    void save('registry-diff.csv', toCsv(rows))
  }

  const exportJson = () => {
    const payload = mode === 'diff' && diffOut ? diffOut.report : doc
    if (!payload) return
    void save(mode === 'diff' ? 'registry-diff.json' : 'registry-view.json', JSON.stringify(payload, null, 2))
  }

  const save = async (filename: string, text: string) => {
    const target = await host.dialog.saveFile(filename)
    if (!target) return
    await host.fs.writeText(target, text)
    toast.success(t('export.saved', { name: filename }))
  }

  const diffKindBadge = (kind: DiffKind) =>
    ({ keyAdded: 'success', keyMissing: 'danger', valueAdded: 'success', valueMissing: 'danger', typeChanged: 'warning', dataChanged: 'warning' })[
      kind
    ] as 'success' | 'danger' | 'warning' | undefined

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-1.5">
        <SegmentedControl<Mode>
          size="sm"
          value={mode}
          onValueChange={(next) => {
            setMode(next)
            setSelected(null)
          }}
          aria-label={t('mode.label')}
          options={[
            { value: 'view', label: t('mode.view') },
            { value: 'diff', label: t('mode.diff') },
          ]}
        />
        <div className="ml-auto flex items-center gap-1.5">
          <Tooltip content={t('export.json')}>
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={t('export.json')}
              disabled={mode === 'diff' ? !diffOut : !doc}
              onClick={exportJson}
            >
              <FileJson />
            </Button>
          </Tooltip>
          {mode === 'diff' && (
            <Tooltip content={t('export.csv')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('export.csv')} disabled={!diffOut} onClick={exportDiffCsv}>
                <FileSpreadsheet />
              </Button>
            </Tooltip>
          )}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,1fr)_minmax(0,1.4fr)] gap-3">
        {/* 左列：文件与键列表 */}
        <div className="flex min-h-0 flex-col gap-3">
          {mode === 'view' ? (
            <Panel
              icon={<KeyRound />}
              title={t('file.title')}
              className="shrink-0"
              bodyClassName="p-3"
              actions={
                <Button size="sm" onClick={() => void pick(setPath)} disabled={parsed.pending}>
                  {parsed.pending ? <Spinner /> : <Search />}
                  {t('file.open')}
                </Button>
              }
            >
              <div className="truncate text-[12.5px] text-fg-muted" data-selectable>
                {path ?? t('file.none')}
              </div>
              {doc && (
                <div className="mt-2 flex gap-1.5">
                  <Badge>{t('file.keys', { count: doc.keys.length })}</Badge>
                  <Badge>{t('file.values', { count: parsed.result?.valueCount ?? 0 })}</Badge>
                  {doc.issues.length > 0 && <Badge variant="warning">{t('file.issues', { count: doc.issues.length })}</Badge>}
                </div>
              )}
              {parsed.error && <p className="mt-2 text-xs text-danger">{errorMessage(parsed.error)}</p>}
            </Panel>
          ) : (
            <Panel icon={<GitCompareArrows />} title={t('diff.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
              {[
                ['A', pathA, setPathA],
                ['B', pathB, setPathB],
              ].map(([label, value, setter]) => (
                <div key={label as string} className="flex items-center gap-2">
                  <Badge variant="info">{label as string}</Badge>
                  <span className="min-w-0 flex-1 truncate text-[12.5px] text-fg-muted" data-selectable>
                    {(value as string | null) ?? t('file.none')}
                  </span>
                  <Button size="sm" variant="outline" onClick={() => void pick(setter as (p: string | null) => void)}>
                    {t('file.open')}
                  </Button>
                </div>
              ))}
              <Button block variant="primary" disabled={!pathA || !pathB || task.running} onClick={runDiff}>
                {task.running ? <Spinner /> : <GitCompareArrows />}
                {t('diff.run')}
              </Button>
              {task.running && <p className="text-xs text-fg-muted tabular-nums">{t('diff.running')}</p>}
              {task.status === 'failed' && task.error && <p className="text-xs text-danger">{errorMessage(task.error)}</p>}
              {diffOut && (
                <div className="space-y-1.5 text-xs">
                  {diffOut.report.identical ? (
                    <p className="text-success">{t('diff.identical')}</p>
                  ) : (
                    <p className="text-fg-muted">{t('diff.count', { count: diffOut.report.entries.length })}</p>
                  )}
                  {diffOut.report.hasDeleteStatements && (
                    <p className="flex items-start gap-1 text-warning">
                      <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
                      {t('diff.deleteHint')}
                    </p>
                  )}
                </div>
              )}
            </Panel>
          )}

          <Panel icon={<Search />} title={t('keys.title')} className="min-h-0 flex-1" bodyClassName="space-y-2 p-2">
            <Input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder={t('keys.filter')} aria-label={t('keys.filter')} className="h-8 text-xs" />
            <ul className="space-y-0.5 overflow-auto" style={{ maxHeight: 'calc(100% - 40px)' }}>
              {keys.length === 0 && <li className="p-3 text-xs text-fg-subtle">{t('keys.empty')}</li>}
              {keys.map((key) => (
                <li key={key.path + key.line}>
                  <button
                    type="button"
                    onClick={() => setSelected(key.path)}
                    className={cn(
                      'block w-full truncate rounded-control px-2 py-1.5 text-left font-mono text-[12px] outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring',
                      selectedKey?.path === key.path && selectedKey?.line === key.line ? 'bg-active text-fg' : 'text-fg-muted',
                    )}
                    title={`${key.path} · L${key.line}`}
                  >
                    {key.delete && <TriangleAlert className="mr-1 inline size-3 text-warning" />}
                    {key.path}
                  </button>
                </li>
              ))}
            </ul>
          </Panel>
        </div>

        {/* 右列：值明细或差异列表 */}
        <Panel
          icon={<KeyRound />}
          title={mode === 'diff' ? t('diff.title') : selectedKey?.path ?? t('values.title')}
          bodyClassName="min-h-0 overflow-auto p-3"
          footer={
            selectedKey && mode === 'view' ? (
              <span className="text-xs text-fg-subtle tabular-nums">{t('values.keyLine', { line: selectedKey.line })}</span>
            ) : null
          }
        >
          {mode === 'diff' ? (
            !diffOut ? (
              <Empty icon={<GitCompareArrows />} title={t('diff.empty')} description={t('diff.emptyHint')} />
            ) : diffOut.report.entries.length === 0 ? (
              <Empty icon={<GitCompareArrows />} title={t('diff.identical')} />
            ) : (
              <ul className="space-y-2">
                {diffOut.report.entries.map((entry, index) => (
                  <li key={index} className="space-y-1 rounded-card border border-border p-2.5">
                    <div className="flex flex-wrap items-center gap-1.5">
                      <Badge variant={diffKindBadge(entry.kind)}>{t(`diffKind.${entry.kind}`)}</Badge>
                      <span className="font-mono text-[12px] text-fg" data-selectable>
                        {entry.path}
                      </span>
                      {entry.valueName !== null && (
                        <span className="font-mono text-[12px] text-fg-muted" data-selectable>
                          · {entry.valueName || t('value.default')}
                        </span>
                      )}
                    </div>
                    <div className="grid gap-1 text-[12.5px]">
                      {entry.a !== null && (
                        <p className="break-all font-mono text-fg-muted">
                          <Badge>A</Badge> <span data-selectable>{entry.a}</span>
                          {entry.lineA && <span className="ml-1 text-fg-subtle">L{entry.lineA}</span>}
                        </p>
                      )}
                      {entry.b !== null && (
                        <p className="break-all font-mono text-fg-muted">
                          <Badge variant="info">B</Badge> <span data-selectable>{entry.b}</span>
                          {entry.lineB && <span className="ml-1 text-fg-subtle">L{entry.lineB}</span>}
                        </p>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            )
          ) : !selectedKey ? (
            <Empty icon={<KeyRound />} title={t('values.empty')} description={t('values.emptyHint')} />
          ) : selectedKey.delete ? (
            <Empty icon={<TriangleAlert />} title={t('value.deleteKey')} description={t('diff.deleteHint')} />
          ) : selectedKey.values.length === 0 ? (
            <Empty icon={<KeyRound />} title={t('values.none')} />
          ) : (
            <ul className="space-y-2">
              {selectedKey.values.map((value, index) => (
                <ValueRow key={`${value.line}-${index}`} value={value} />
              ))}
            </ul>
          )}
        </Panel>
      </div>
    </div>
  )
}

function ValueRow({ value }: { value: ValueEntry }) {
  const { t, errorMessage } = usePlugin()
  return (
    <li className="space-y-1.5 rounded-card border border-border p-2.5">
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="font-mono text-[12.5px] text-fg" data-selectable>
          {value.isDefault ? t('value.default') : value.name}
        </span>
        {value.isDefault && <Badge>{t('value.default')}</Badge>}
        <ValueBadge value={value.value} />
        {value.duplicate && <Badge variant="warning">{t('value.duplicate')}</Badge>}
        <span className="ml-auto text-[11px] text-fg-subtle tabular-nums">L{value.line}</span>
      </div>
      <p className="break-all font-mono text-[12.5px] text-fg-muted" data-selectable>
        {value.value.kind === 'multiSz' ? (value.value.items ?? []).join(' ⏎ ') : displayValue(value.value)}
      </p>
      {value.issues.map((issue) => (
        <p key={issue} className="text-[11.5px] text-warning">
          {errorMessage({ code: issue })}
        </p>
      ))}
    </li>
  )
}

function ValueBadge({ value }: { value: RegValue }) {
  const { t } = usePlugin()
  if (value.kind === 'unknown') {
    return value.typeId === 4294967295 ? (
      <Badge variant="danger">{t('value.deleted')}</Badge>
    ) : (
      <Badge variant="warning">{t('value.unknownType', { id: value.typeId ?? 0 })}</Badge>
    )
  }
  const names: Record<string, string> = {
    sz: 'REG_SZ',
    expandSz: 'REG_EXPAND_SZ',
    multiSz: 'REG_MULTI_SZ',
    dword: 'REG_DWORD',
    qword: 'REG_QWORD',
    binary: 'REG_BINARY',
  }
  return <Badge variant="info">{names[value.kind] ?? value.kind}</Badge>
}

function displayValue(value: RegValue): string {
  switch (value.kind) {
    case 'sz':
    case 'expandSz':
      return value.text ?? ''
    case 'dword':
      return `0x${(value.value ?? 0).toString(16).padStart(8, '0')} (${value.value})`
    case 'qword':
      return `0x${(value.value ?? 0).toString(16).padStart(16, '0')} (${value.value})`
    case 'binary':
      return (value.bytes ?? []).map((b) => b.toString(16).padStart(2, '0')).join(' ')
    case 'unknown':
      return (value.bytes ?? []).map((b) => b.toString(16).padStart(2, '0')).join(' ')
    default:
      return ''
  }
}
