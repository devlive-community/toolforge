import { useMemo, useRef, useState } from 'react'
import {
  Button,
  CodeEditor,
  DropdownMenu,
  Input,
  Panel,
  Spinner,
  Tabs,
  Tooltip,
  cn,
  type CodeEditorHandle,
  type EditorMark,
} from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BookMarked, ChevronDown, FileText, ListFilter, Replace, TriangleAlert } from 'lucide-react'
import { MatchList } from './MatchList'
import { PRESETS, SAMPLE_TEXT } from './presets'
import { FLAG_KEYS, type Flags, type ReplaceResult, type TestResult } from './types'

export function RegexTool() {
  const { t, errorMessage } = usePlugin()
  const [pattern, setPattern] = useState(PRESETS[0].pattern)
  const [flags, setFlags] = useState<Flags>({ caseInsensitive: false, multiLine: false, dotAll: false, extended: false })
  const [input, setInput] = useState(SAMPLE_TEXT)
  const [tab, setTab] = useState<'matches' | 'replace'>('matches')
  const [replacement, setReplacement] = useState('<$0>')
  const [active, setActive] = useState<number | null>(null)
  const editor = useRef<CodeEditorHandle>(null)

  const ready = pattern.length > 0
  const flagKey = FLAG_KEYS.map(({ key }) => (flags[key] ? 1 : 0)).join('')
  const test = useDebouncedCall<TestResult>(ready ? 'test' : null, ready ? { pattern, flags, input } : null, [pattern, flagKey, input])
  const replace = useDebouncedCall<ReplaceResult>(
    ready && tab === 'replace' ? 'replace' : null,
    { pattern, flags, input, replacement },
    [pattern, flagKey, input, replacement, tab],
  )

  const result = test.error ? null : test.result
  const marks = useMemo<EditorMark[]>(
    () =>
      (result?.matches ?? [])
        .filter((m) => m.end > m.start)
        .map((m, i) => ({ from: m.start, to: m.end, tone: i === active ? 'active' : i % 2 ? 'alt' : 'primary' })),
    [result, active],
  )

  const select = (index: number) => {
    const match = result?.matches[index]
    if (!match) return
    setActive(index)
    editor.current?.selectRange(match.start, match.end)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="shrink-0 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex items-center gap-2">
          <span className="font-mono text-xl text-fg-subtle">/</span>
          <Input
            value={pattern}
            onChange={(event) => {
              setPattern(event.target.value)
              setActive(null)
            }}
            placeholder={t('pattern.placeholder')}
            className="font-mono text-[14px]"
            wrapperClassName="flex-1"
            size="lg"
            invalid={!!test.error}
            aria-label={t('pattern.label')}
          />
          <span className="font-mono text-xl text-fg-subtle">/</span>
          <div className="flex items-center gap-1" role="group" aria-label={t('flags.label')}>
            {FLAG_KEYS.map(({ key, letter }) => (
              <Tooltip key={key} content={t(`flags.${key}`)}>
                <Button
                  size="icon-md"
                  variant={flags[key] ? 'soft' : 'outline'}
                  aria-pressed={flags[key]}
                  aria-label={t(`flags.${key}`)}
                  className={cn('font-mono text-[13px]', flags[key] && 'ring-1 ring-primary/40')}
                  onClick={() => setFlags((prev) => ({ ...prev, [key]: !prev[key] }))}
                >
                  {letter}
                </Button>
              </Tooltip>
            ))}
          </div>
          <DropdownMenu
            className="max-h-80 overflow-auto"
            trigger={
              <Button>
                <BookMarked />
                {t('presets.label')}
                <ChevronDown />
              </Button>
            }
            items={PRESETS.map((preset) => ({
              key: preset.key,
              label: t(`presets.${preset.key}`),
              onSelect: () => {
                setPattern(preset.pattern)
                setActive(null)
              },
            }))}
          />
        </div>
        {test.error && (
          <p className="mt-2 flex items-start gap-1.5 text-xs text-danger">
            <TriangleAlert className="mt-px size-3.5 shrink-0" />
            <span className="font-mono break-all whitespace-pre-wrap" data-selectable>
              {errorMessage(test.error)}
              {typeof test.error.params?.detail === 'string' && ` — ${test.error.params.detail}`}
            </span>
          </p>
        )}
      </section>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.25fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<FileText />}
          title={t('input.title')}
          extra={test.pending && <Spinner className="size-3.5 text-fg-subtle" />}
          footer={
            result && (
              <>
                <span>{t('status.count', { count: result.count })}</span>
                {result.truncated && <span className="text-warning">{t('status.truncated', { shown: result.matches.length })}</span>}
                <span className="ml-auto">{t('status.elapsed', { ms: result.elapsedMs })}</span>
              </>
            )
          }
        >
          <CodeEditor
            ref={editor}
            value={input}
            onChange={(value) => {
              setInput(value)
              setActive(null)
            }}
            language="text"
            lineWrapping
            marks={marks}
            placeholder={t('input.placeholder')}
            aria-label={t('input.title')}
          />
        </Panel>

        <Panel bodyClassName="flex flex-col">
          <div className="shrink-0 border-b border-border px-2 pt-2">
            <Tabs
              variant="underline"
              value={tab}
              onValueChange={setTab}
              items={[
                { value: 'matches', label: t('matches.title', { count: result?.count ?? 0 }), icon: <ListFilter /> },
                { value: 'replace', label: t('replace.title'), icon: <Replace /> },
              ]}
            />
          </div>
          {tab === 'matches' ? (
            <div className="min-h-0 flex-1 overflow-auto">
              {result && <MatchList matches={result.matches} active={active} onSelect={select} />}
            </div>
          ) : (
            <div className="flex min-h-0 flex-1 flex-col gap-2 p-3">
              <div className="flex shrink-0 items-center gap-2">
                <Input
                  value={replacement}
                  onChange={(event) => setReplacement(event.target.value)}
                  placeholder={t('replace.placeholder')}
                  className="font-mono"
                  wrapperClassName="flex-1"
                  aria-label={t('replace.label')}
                />
                {replace.result && <CopyButton text={replace.result.output} label={t('replace.copy')} variant="outline" />}
              </div>
              <p className="shrink-0 text-xs text-fg-muted">{t('replace.hint')}</p>
              <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border">
                {replace.error ? (
                  <p className="p-3 text-xs text-danger">{errorMessage(replace.error)}</p>
                ) : (
                  <CodeEditor value={replace.result?.output ?? ''} readOnly language="text" lineWrapping aria-label={t('replace.output')} />
                )}
              </div>
              {replace.result && <p className="shrink-0 text-xs text-fg-muted">{t('replace.count', { count: replace.result.count })}</p>}
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}
