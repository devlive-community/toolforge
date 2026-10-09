import { useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, Panel, Select, Switch, Tooltip, cn, toast, type CodeLanguage } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, useLaunchInput, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { AlignCenter, AlignLeft, AlignRight, ArrowRightLeft, ClipboardPaste, Minus, Table2, X } from 'lucide-react'

type Source = 'markdown' | 'html' | 'json' | 'tsv' | 'csv'
type Target = 'markdown' | 'csv' | 'tsv' | 'html' | 'json' | 'ascii' | 'sql' | 'latex'
type Align = 'none' | 'left' | 'center' | 'right'
interface Table {
  headers: string[]
  rows: string[][]
  aligns: Align[]
}
interface Converted {
  output: string
  detected: Source
  header: boolean
  columns: number
  rows: number
  preview: Table
  truncated: boolean
}

const SOURCES: Source[] = ['markdown', 'csv', 'tsv', 'json', 'html']
const TARGETS: Target[] = ['markdown', 'csv', 'tsv', 'html', 'json', 'ascii', 'sql', 'latex']
const LANGUAGE: Record<Target, CodeLanguage> = {
  markdown: 'markdown',
  csv: 'text',
  tsv: 'text',
  html: 'xml',
  json: 'json',
  ascii: 'text',
  sql: 'sql',
  latex: 'text',
}
const NEXT_ALIGN: Record<Align, Align> = { none: 'left', left: 'center', center: 'right', right: 'none' }
const ALIGN_ICON = { none: Minus, left: AlignLeft, center: AlignCenter, right: AlignRight }
const ALIGN_CLASS: Record<Align, string> = { none: 'text-left', left: 'text-left', center: 'text-center', right: 'text-right' }

const SAMPLE = `城市\t人口（万）\t面积（km²）
北京\t2189\t16410
上海\t2487\t6340
广州\t1882\t7434
深圳\t1779\t1997`

function TableConverter() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState(SAMPLE)
  const [from, setFrom] = useState<Source | 'auto'>('auto')
  const [to, setTo] = usePluginState<Target>('to', 'markdown')
  const [header, setHeader] = useState<boolean | null>(null)
  const [aligns, setAligns] = useState<Align[] | null>(null)
  const [transpose, setTranspose] = useState(false)
  const [compact, setCompact] = usePluginState('compact', false)
  const [tableName, setTableName] = usePluginState('tableName', 'my_table')
  useLaunchInput((text) => {
    setInput(text)
    setHeader(null)
    setAligns(null)
  })

  const args = input.trim()
    ? { input, from: from === 'auto' ? null : from, to, header, transpose, trim: true, aligns, compact, tableName }
    : null
  const result = useDebouncedCall<Converted>(args ? 'convert' : null, args, [input, from, to, header, transpose, aligns, compact, tableName])
  const r = result.result

  const changeInput = (text: string) => {
    setInput(text)
    setAligns(null)
  }
  const paste = async () => {
    try {
      const text = await host.clipboard.readText()
      if (text) {
        changeInput(text)
        setHeader(null)
      }
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const cycleAlign = (column: number) => {
    if (!r) return
    const current = aligns ?? r.preview.aligns
    setAligns(current.map((a, i) => (i === column ? NEXT_ALIGN[a] : a)))
  }
  const columnAligns = aligns ?? r?.preview.aligns ?? []

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="grid min-h-0 flex-[3] grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<Table2 />}
          title={t('input.title')}
          extra={r && from === 'auto' && <Badge variant="info">{t(`sources.${r.detected}`)}</Badge>}
          actions={
            <>
              <Select<Source | 'auto'>
                size="sm"
                value={from}
                onValueChange={setFrom}
                aria-label={t('input.from')}
                options={[{ value: 'auto', label: t('input.auto') }, ...SOURCES.map((value) => ({ value, label: t(`sources.${value}`) }))]}
              />
              <Button size="sm" onClick={paste}>
                <ClipboardPaste />
                {t('input.paste')}
              </Button>
              <Button size="icon-sm" variant="ghost" aria-label={t('input.clear')} onClick={() => changeInput('')}>
                <X />
              </Button>
            </>
          }
          bodyClassName="p-0"
        >
          <CodeEditor value={input} onChange={changeInput} language="text" placeholder={t('input.placeholder')} className="h-full" aria-label={t('input.title')} />
        </Panel>

        <Panel
          icon={<ArrowRightLeft />}
          title={t('output.title')}
          actions={
            <>
              <Select<Target>
                size="sm"
                value={to}
                onValueChange={setTo}
                aria-label={t('output.to')}
                options={TARGETS.map((value) => ({ value, label: t(`targets.${value}`) }))}
              />
              {r && <CopyButton text={r.output} label={t('output.copy')} />}
            </>
          }
          bodyClassName="p-0"
        >
          {result.error ? (
            <p className="p-3 text-[13px] text-danger">{errorMessage(result.error)}</p>
          ) : (
            <CodeEditor value={r?.output ?? ''} language={LANGUAGE[to]} readOnly className={cn('h-full', result.pending && 'opacity-60')} aria-label={t('output.title')} />
          )}
        </Panel>
      </div>

      <div className="flex shrink-0 flex-wrap items-center gap-x-5 gap-y-2 rounded-card border border-border bg-surface px-3 py-2 text-xs text-fg-muted">
        <label className="flex items-center gap-1.5">
          <Switch size="sm" checked={r?.header ?? true} onCheckedChange={setHeader} aria-label={t('options.header')} />
          {t('options.header')}
        </label>
        <label className="flex items-center gap-1.5">
          <Switch size="sm" checked={transpose} onCheckedChange={setTranspose} aria-label={t('options.transpose')} />
          {t('options.transpose')}
        </label>
        {to === 'markdown' && (
          <label className="flex items-center gap-1.5">
            <Switch size="sm" checked={compact} onCheckedChange={setCompact} aria-label={t('options.compact')} />
            {t('options.compact')}
          </label>
        )}
        {to === 'sql' && (
          <label className="flex items-center gap-1.5">
            {t('options.tableName')}
            <Input size="sm" value={tableName} onChange={(e) => setTableName(e.target.value)} className="font-mono" wrapperClassName="w-40" aria-label={t('options.tableName')} />
          </label>
        )}
        {r && <span className="ml-auto tabular-nums">{t('options.size', { rows: r.rows, columns: r.columns })}</span>}
      </div>

      <Panel title={t('preview.title')} extra={<span className="text-[11px] text-fg-subtle">{t('preview.hint')}</span>} className="min-h-0 flex-[2]" bodyClassName="overflow-auto">
        {!r ? (
          <Empty icon={<Table2 />} title={t('preview.empty')} />
        ) : (
          <table className="w-full border-collapse text-[12.5px]">
            <thead className="sticky top-0 bg-surface-2">
              <tr>
                {Array.from({ length: r.columns }, (_, c) => {
                  const align = columnAligns[c] ?? 'none'
                  const Icon = ALIGN_ICON[align]
                  return (
                    <th key={c} className={cn('border-b border-border px-3 py-1.5 font-medium text-fg', ALIGN_CLASS[align])}>
                      <Tooltip content={t('preview.align', { align: t(`aligns.${align}`) })}>
                        <button type="button" className="inline-flex max-w-full items-center gap-1 rounded-control px-1 hover:bg-hover" onClick={() => cycleAlign(c)}>
                          <span className="truncate">{r.preview.headers[c] ?? ''}</span>
                          <Icon className="size-3.5 shrink-0 text-fg-subtle" />
                        </button>
                      </Tooltip>
                    </th>
                  )
                })}
              </tr>
            </thead>
            <tbody>
              {r.preview.rows.map((row, i) => (
                <tr key={i} className="hover:bg-hover">
                  {row.map((cell, c) => (
                    <td key={c} className={cn('border-b border-border px-3 py-1.5 text-fg', ALIGN_CLASS[columnAligns[c] ?? 'none'])} data-selectable>
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {r?.truncated && <p className="p-2 text-[11px] text-fg-subtle">{t('preview.truncated')}</p>}
      </Panel>
    </div>
  )
}

export default TableConverter
