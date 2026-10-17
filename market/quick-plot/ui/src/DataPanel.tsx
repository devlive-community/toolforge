import { useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Panel, Select, Spinner, Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileText, FolderOpen, Sparkles, Table2, X } from 'lucide-react'
import type { Format, HeaderMode, Kind, Summary } from './types'

type DataTab = 'input' | 'preview'

export type Source = { kind: 'text' } | { kind: 'file'; path: string }

const KIND_TONE: Record<Kind, 'primary' | 'info' | 'neutral'> = { number: 'primary', time: 'info', text: 'neutral' }

export function DataPanel({
  text,
  onText,
  source,
  format,
  onFormat,
  header,
  onHeader,
  summary,
  error,
  busy,
  onOpen,
  onSample,
  onCloseFile,
}: {
  text: string
  onText: (text: string) => void
  source: Source
  format: Format
  onFormat: (format: Format) => void
  header: HeaderMode
  onHeader: (header: HeaderMode) => void
  summary: Summary | null
  error: string | null
  busy: boolean
  onOpen: () => void
  onSample: () => void
  onCloseFile: () => void
}) {
  const { t } = usePlugin()
  const [tab, setTab] = useState<DataTab>('input')
  const json = summary?.format === 'json' || summary?.format === 'jsonl'

  const detected = summary && (
    <>
      <span className="shrink-0">{t(`formats.${summary.format}`)}</span>
      {summary.delimiter && <span className="shrink-0">{t(`delimiters.${summary.delimiter}`)}</span>}
      {!json && <span className="shrink-0">{summary.header ? t('data.withHeader') : t('data.noHeader')}</span>}
      {summary.jsonPath && <span className="min-w-0 truncate font-mono">{summary.jsonPath}</span>}
      <span className="ml-auto shrink-0 tabular-nums">{t('data.size', { rows: summary.rows.toLocaleString(), columns: summary.columns.length })}</span>
    </>
  )

  return (
    <Panel
      title={
        <Tabs<DataTab>
          variant="underline"
          value={tab}
          onValueChange={setTab}
          aria-label={t('data.title')}
          items={[
            { value: 'input', label: t('data.input') },
            { value: 'preview', label: t('data.preview') },
          ]}
        />
      }
      extra={busy && <Spinner className="size-3.5 text-fg-subtle" />}
      footer={error ? <span className="min-w-0 truncate text-danger">{error}</span> : detected}
      bodyClassName="flex min-h-0 flex-col"
    >
      {tab === 'input' ? (
        <>
          <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-border px-3 py-2" title={t('data.dropHint')}>
            <Button size="sm" variant="outline" onClick={onOpen}>
              <FolderOpen />
              {t('data.open')}
            </Button>
            <Button size="sm" variant="ghost" onClick={onSample}>
              <Sparkles />
              {t('data.sample')}
            </Button>
            <span className="ml-auto" />
            <Select<Format>
              size="sm"
              value={format}
              onValueChange={onFormat}
              className="w-28"
              aria-label={t('data.format')}
              options={(['auto', 'csv', 'tsv', 'json', 'jsonl'] as Format[]).map((value) => ({ value, label: t(`formats.${value}`) }))}
            />
            {!json && (
              <Select<HeaderMode>
                size="sm"
                value={header}
                onValueChange={onHeader}
                className="w-28"
                aria-label={t('data.header')}
                options={(['auto', 'yes', 'no'] as HeaderMode[]).map((value) => ({ value, label: t(`headers.${value}`) }))}
              />
            )}
          </div>
          {source.kind === 'file' ? (
            <Empty
              icon={<FileText />}
              title={summary?.file ?? source.path}
              description={t('data.fileHint')}
              action={
                <Button size="sm" variant="outline" onClick={onCloseFile}>
                  <X />
                  {t('data.closeFile')}
                </Button>
              }
            />
          ) : (
            <div className="min-h-0 flex-1">
              <CodeEditor value={text} onChange={onText} language={text.trimStart().startsWith('{') || text.trimStart().startsWith('[') ? 'json' : 'text'} placeholder={t('data.placeholder')} aria-label={t('data.input')} />
            </div>
          )}
        </>
      ) : !summary ? (
        <Empty icon={<Table2 />} title={t('data.noPreview')} />
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-max min-w-full text-left text-xs">
            <thead className="sticky top-0 bg-surface-2">
              <tr>
                {summary.columns.map((column) => (
                  <th key={column.index} className="border-b border-border px-3 py-2 align-top font-medium text-fg">
                    <span className="block max-w-48 truncate" title={column.name}>
                      {column.name}
                    </span>
                    <Badge variant={KIND_TONE[column.kind]} className="mt-1">
                      {t(`kinds.${column.kind}`)}
                    </Badge>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="font-mono">
              {summary.preview.map((row, i) => (
                <tr key={i} className="border-b border-border">
                  {row.map((cell, j) => (
                    <td key={j} className="max-w-64 truncate px-3 py-1.5 text-fg-muted" title={cell} data-selectable>
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
          {summary.rows > summary.preview.length && <p className="px-3 py-2 text-xs text-fg-subtle">{t('data.previewLimit', { shown: summary.preview.length, rows: summary.rows.toLocaleString() })}</p>}
        </div>
      )}
    </Panel>
  )
}
