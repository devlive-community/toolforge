import { useEffect, useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Switch, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin, usePluginState, type AppError } from '@toolforge/plugin-ui-sdk'
import { BookOpen, Braces, FileJson, FolderOpen, Search, X } from 'lucide-react'

type Language = 'jq' | 'jsonpath'
interface Output {
  path: string | null
  text: string
}
interface Answer {
  results: Output[]
  truncated: boolean
  inputs: number
  elapsedMs: number
}

/** 示例查询（代码本身不翻译，说明文字按 key 翻译） */
const EXAMPLES: Record<Language, { key: string; query: string }[]> = {
  jq: [
    { key: 'field', query: '.users[0].name' },
    { key: 'each', query: '.users[] | .name' },
    { key: 'filter', query: '.users[] | select(.age >= 18)' },
    { key: 'map', query: '.users | map({name, city})' },
    { key: 'count', query: '.users | length' },
    { key: 'average', query: '[.users[].age] | add / length' },
    { key: 'group', query: '.users | group_by(.city) | map({city: .[0].city, count: length})' },
    { key: 'sort', query: '.users | sort_by(.age) | reverse' },
    { key: 'keys', query: 'keys' },
    { key: 'text', query: '.users[] | "\\(.name) (\\(.age))"' },
    { key: 'paths', query: '[paths(scalars)] | map(join("."))' },
  ],
  jsonpath: [
    { key: 'field', query: '$.users[0].name' },
    { key: 'each', query: '$.users[*].name' },
    { key: 'filter', query: '$.users[?@.age >= 18]' },
    { key: 'deep', query: '$..name' },
    { key: 'slice', query: '$.users[0:2]' },
    { key: 'last', query: '$.users[-1]' },
  ],
}

const SAMPLE = `{
  "users": [
    { "name": "Ann", "age": 31, "city": "Beijing", "tags": ["admin", "dev"] },
    { "name": "Bob", "age": 17, "city": "Shanghai", "tags": [] },
    { "name": "Cid", "age": 45, "city": "Beijing", "tags": ["dev"] }
  ],
  "meta": { "version": "1.2", "count": 3 }
}`

function JsonQuery() {
  const { t, errorMessage } = usePlugin()
  const [language, setLanguage] = usePluginState<Language>('language', 'jq')
  const [query, setQuery] = usePluginState('query', '.users[] | select(.age >= 18) | .name')
  const [raw, setRaw] = usePluginState('raw', false)
  const [compact, setCompact] = usePluginState('compact', false)
  const [sortKeys, setSortKeys] = usePluginState('sortKeys', false)
  const [text, setText] = useState(SAMPLE)
  const [file, setFile] = useState<{ path: string; name: string } | null>(null)

  const source = file ? { path: file.path } : { text }
  const args = query.trim() && (file || text.trim()) ? { source, query, language, raw, compact, sortKeys } : null
  const answer = useDebouncedCall<Answer>(args ? 'query' : null, args, [file?.path, text, query, language, raw, compact, sortKeys])
  const error = answer.error as AppError | null
  const jsonErrorLine = error?.code === 'query.invalid_json' && !file ? Number(error.params?.line) || null : null

  useEffect(() => {
    const off = host.onFileDrop({
      drop: (paths) => {
        const path = paths[0]
        if (path) setFile({ path, name: path.split(/[\\/]/).pop() ?? path })
      },
    })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const openFile = async () => {
    try {
      const path = await host.dialog.openFile([{ name: 'JSON', extensions: ['json', 'jsonl', 'ndjson', 'geojson', 'txt'] }])
      if (path) setFile({ path, name: path.split(/[\\/]/).pop() ?? path })
    } catch (e) {
      toast.error(errorMessage(e))
    }
  }
  const results = answer.result?.results ?? []
  const all = results.map((r) => r.text).join('\n')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<Search />} title={t('query.title')} className="shrink-0" bodyClassName="space-y-2.5 p-3">
        <div className="flex items-center gap-2">
          <SegmentedControl<Language>
            value={language}
            onValueChange={setLanguage}
            aria-label={t('query.language')}
            options={[
              { value: 'jq', label: 'jq' },
              { value: 'jsonpath', label: 'JSONPath' },
            ]}
          />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t(`query.placeholder.${language}`)}
            className="font-mono"
            wrapperClassName="min-w-0 flex-1"
            invalid={!!error && error.code !== 'query.invalid_json'}
            aria-label={t('query.title')}
            autoFocus
          />
          <DropdownMenu
            trigger={
              <Button>
                <BookOpen />
                {t('query.examples')}
              </Button>
            }
            items={EXAMPLES[language].map((example) => ({
              key: example.key,
              label: t(`examples.${example.key}`),
              hint: <code className="font-mono text-[11px]">{example.query}</code>,
              onSelect: () => setQuery(example.query),
            }))}
          />
        </div>
        <div className="flex flex-wrap items-center gap-4 text-xs text-fg-muted">
          {[
            { label: t('options.raw'), checked: raw, set: setRaw },
            { label: t('options.compact'), checked: compact, set: setCompact },
            { label: t('options.sortKeys'), checked: sortKeys, set: setSortKeys },
          ].map((option) => (
            <label key={option.label} className="flex items-center gap-1.5">
              <Switch size="sm" checked={option.checked} onCheckedChange={option.set} aria-label={option.label} />
              {option.label}
            </label>
          ))}
          {error && error.code !== 'query.invalid_json' && <span className="ml-auto text-danger">{errorMessage(error)}</span>}
        </div>
      </Panel>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<FileJson />}
          title={t('input.title')}
          actions={
            <>
              <Button size="sm" onClick={openFile}>
                <FolderOpen />
                {t('input.open')}
              </Button>
              {!file && (
                <Button size="icon-sm" variant="ghost" aria-label={t('input.clear')} onClick={() => setText('')}>
                  <X />
                </Button>
              )}
            </>
          }
          footer={jsonErrorLine && error ? <span className="text-danger">{errorMessage(error)}</span> : undefined}
          bodyClassName="p-0"
        >
          {file ? (
            <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
              <FileJson className="size-8 text-fg-subtle" />
              <p className="text-[13px] font-medium text-fg">{file.name}</p>
              <p className="max-w-full truncate text-[11px] text-fg-subtle" title={file.path}>
                {file.path}
              </p>
              {error?.code === 'query.invalid_json' && <p className="text-xs text-danger">{errorMessage(error)}</p>}
              <Button size="sm" variant="ghost" onClick={() => setFile(null)}>
                {t('input.backToText')}
              </Button>
            </div>
          ) : (
            <CodeEditor value={text} onChange={setText} language="json" errorLine={jsonErrorLine} placeholder={t('input.placeholder')} className="h-full" aria-label={t('input.title')} />
          )}
        </Panel>

        <Panel
          icon={<Braces />}
          title={t('result.title')}
          extra={
            answer.result && (
              <>
                <Badge>{t('result.count', { count: results.length })}</Badge>
                {answer.result.inputs > 1 && <Badge variant="info">{t('result.lines', { count: answer.result.inputs })}</Badge>}
                <span className="text-[11px] text-fg-subtle tabular-nums">{t('result.elapsed', { ms: answer.result.elapsedMs })}</span>
              </>
            )
          }
          actions={results.length > 0 && <CopyButton text={all} label={t('result.copyAll')} />}
          footer={answer.result?.truncated ? <span className="text-warning">{t('result.truncated')}</span> : undefined}
          bodyClassName={cn('overflow-auto p-2', answer.pending && 'opacity-60')}
        >
          {!args ? (
            <Empty icon={<Braces />} title={t('result.empty')} description={t('result.emptyHint')} />
          ) : answer.result && results.length === 0 ? (
            <Empty icon={<Braces />} title={t('result.none')} />
          ) : (
            <div className="space-y-1.5">
              {results.map((r, i) => (
                <div key={i} className="group relative rounded-control border border-border bg-surface-2">
                  {r.path && <p className="border-b border-border px-2.5 py-1 font-mono text-[11px] text-fg-subtle">{r.path}</p>}
                  <pre className="overflow-x-auto px-2.5 py-1.5 font-mono text-[12px] leading-relaxed text-fg" data-selectable>
                    {r.text}
                  </pre>
                  <CopyButton text={r.text} className="absolute top-1 right-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                </div>
              ))}
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}

export default JsonQuery
