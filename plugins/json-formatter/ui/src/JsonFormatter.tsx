import { useRef, useState, type ReactNode } from 'react'
import {
  Button,
  CodeEditor,
  DropdownMenu,
  Panel,
  SegmentedControl,
  Select,
  Spinner,
  Switch,
  Tabs,
  Tooltip,
  cn,
  toast,
  type CodeEditorHandle,
  type CursorPosition,
} from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import {
  ArrowDownAZ,
  Binary,
  ChevronDown,
  ClipboardPaste,
  Copy,
  Download,
  FileJson,
  FileText,
  FolderOpen,
  GitCompare,
  ListTree,
  Maximize2,
  Minimize2,
  Minimize,
  ShieldCheck,
  Sparkles,
  Terminal,
} from 'lucide-react'
import { DiffResult } from './components/DiffResult'
import { ErrorCard } from './components/ErrorCard'
import { StatsView } from './components/StatsView'
import { TreeView } from './components/TreeView'
import { ValidCard } from './components/ValidCard'
import { SAMPLES } from './samples'
import type { DiffReport, Indent, Mode, ProcessResult, Tab } from './types'

const JSON_FILTERS = [{ name: 'JSON', extensions: ['json', 'json5', 'jsonc', 'txt'] }]

function Footer({ cursor, chars, children }: { cursor?: CursorPosition; chars: number; children?: ReactNode }) {
  const { t } = usePlugin()
  return (
    <>
      {cursor && <span>{t('status.cursor', { line: cursor.line, column: cursor.column })}</span>}
      <span>{t('status.chars', { count: chars })}</span>
      <div className="ml-auto flex items-center gap-3">{children}</div>
    </>
  )
}

function WrapToggle({ checked, onChange }: { checked: boolean; onChange: (value: boolean) => void }) {
  const { t } = usePlugin()
  return (
    <label className="flex items-center gap-2">
      {t('status.wrap')}
      <Switch size="sm" checked={checked} onCheckedChange={onChange} aria-label={t('status.wrap')} />
    </label>
  )
}

export function JsonFormatter() {
  const { t, errorMessage } = usePlugin()
  const [tab, setTab] = useState<Tab>('format')
  const [input, setInput] = useState(SAMPLES[0].text)
  const [right, setRight] = useState(SAMPLES[0].text.replace('"0.1.0"', '"0.2.0"'))
  const [mode, setMode] = useState<Mode>('json')
  const [indent, setIndent] = useState<Indent>('2')
  const [sortKeys, setSortKeys] = useState(false)
  const [escapeDir, setEscapeDir] = useState<'escape' | 'unescape'>('escape')
  const [wrapIn, setWrapIn] = useState(false)
  const [wrapOut, setWrapOut] = useState(false)
  const [side, setSide] = useState<'tree' | 'stats'>('tree')
  const [expanded, setExpanded] = useState(false)
  const [cursorIn, setCursorIn] = useState<CursorPosition>({ line: 1, column: 1 })
  const [cursorOut, setCursorOut] = useState<CursorPosition>({ line: 1, column: 1 })
  const inputRef = useRef<CodeEditorHandle>(null)

  const isEscape = tab === 'escape'
  const fn = tab === 'diff' ? null : tab === 'minify' ? 'minify' : tab === 'validate' ? 'validate' : isEscape ? escapeDir : 'format'
  const hasInput = input.trim().length > 0
  const args = !hasInput ? null : isEscape ? { input } : { input, mode, indent, sortKeys }
  const { result, error, pending } = useDebouncedCall<ProcessResult>(fn, args, [fn, input, mode, indent, sortKeys])
  const diff = useDebouncedCall<DiffReport>(tab === 'diff' ? 'diff' : null, { left: input, right, mode }, [tab, input, right, mode])

  const errorLine = error && !isEscape ? Number(error.params?.line ?? 0) || null : null
  const diffErrorLine =
    diff.error?.params?.side === 'left' ? Number(diff.error.params?.line ?? 0) || null : null
  const rightErrorLine =
    diff.error?.params?.side === 'right' ? Number(diff.error.params?.line ?? 0) || null : null

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action()
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const paste = () =>
    run(async () => {
      const text = await host.clipboard.readText()
      if (text) setInput(text)
    })
  const openFile = () =>
    run(async () => {
      const path = await host.dialog.openFile(JSON_FILTERS)
      if (path) setInput(await host.fs.readText(path))
    })
  const copy = () =>
    run(async () => {
      if (!result) return
      await host.clipboard.writeText(result.output)
      toast.success(t('common:copied'))
    })
  const download = () =>
    run(async () => {
      if (!result) return
      const path = await host.dialog.saveFile(isEscape ? 'output.txt' : 'formatted.json', JSON_FILTERS)
      if (!path) return
      await host.fs.writeText(path, result.output)
      toast.success(t('common:saved'))
    })

  const tabs = [
    { value: 'format' as const, label: t('tabs.format'), icon: <Sparkles /> },
    { value: 'minify' as const, label: t('tabs.minify'), icon: <Minimize /> },
    { value: 'validate' as const, label: t('tabs.validate'), icon: <ShieldCheck /> },
    { value: 'escape' as const, label: t('tabs.escape'), icon: <Binary /> },
    { value: 'tree' as const, label: t('tabs.tree'), icon: <ListTree /> },
    { value: 'diff' as const, label: t('tabs.diff'), icon: <GitCompare /> },
  ]

  const modeSelect = (
    <Select<Mode>
      size="sm"
      variant="ghost"
      value={mode}
      onValueChange={setMode}
      aria-label={t('status.mode')}
      options={[
        { value: 'json', label: 'JSON' },
        { value: 'json5', label: 'JSON5' },
      ]}
    />
  )

  const inputPanel = (
    <Panel
      icon={<FileText />}
      title={tab === 'diff' ? t('input.left') : isEscape ? t('input.text') : t('input.title')}
      extra={
        !isEscape && (
          <DropdownMenu
            placement="bottom-start"
            trigger={
              <Button variant="soft" size="sm" className="ml-1">
                {t('input.samples')}
                <ChevronDown />
              </Button>
            }
            items={SAMPLES.map((sample) => ({
              key: sample.key,
              label: t(`samples.${sample.key}`),
              onSelect: () => {
                setInput(sample.text)
                setMode(sample.mode)
              },
            }))}
          />
        )
      }
      actions={
        <>
          <Button size="sm" onClick={() => setInput('')}>
            {t('common:clear')}
          </Button>
          <Button size="sm" onClick={paste}>
            <ClipboardPaste />
            {t('common:paste')}
          </Button>
          <Tooltip content={t('input.open')}>
            <Button size="icon-sm" aria-label={t('input.open')} onClick={openFile}>
              <FolderOpen />
            </Button>
          </Tooltip>
        </>
      }
      footer={
        <Footer cursor={cursorIn} chars={input.length}>
          <WrapToggle checked={wrapIn} onChange={setWrapIn} />
          {!isEscape && modeSelect}
        </Footer>
      }
    >
      <CodeEditor
        ref={inputRef}
        value={input}
        onChange={setInput}
        language={isEscape ? 'text' : 'json'}
        lineWrapping={wrapIn}
        errorLine={tab === 'diff' ? diffErrorLine : errorLine}
        onCursorChange={setCursorIn}
        placeholder={t('input.placeholder')}
        aria-label={t('input.title')}
      />
    </Panel>
  )

  const outputTitle = isEscape ? t(`output.${escapeDir}`) : t(`output.${tab === 'minify' ? 'minified' : tab === 'validate' ? 'validated' : 'formatted'}`)

  const outputBody = () => {
    if (!hasInput) return <p className="p-6 text-center text-[13px] text-fg-subtle">{t('output.waiting')}</p>
    if (error) return <ErrorCard error={error} onLocate={(line, column) => inputRef.current?.gotoLine(line, column)} />
    if (!result) {
      return (
        <div className="flex h-full items-center justify-center text-fg-muted">
          <Spinner />
        </div>
      )
    }
    if (tab === 'validate' && result.stats) return <ValidCard stats={result.stats} elapsedMs={result.elapsedMs} />
    return (
      <CodeEditor
        value={result.output}
        readOnly
        language={isEscape ? 'text' : 'json'}
        lineWrapping={wrapOut || tab === 'minify'}
        onCursorChange={setCursorOut}
        aria-label={outputTitle}
      />
    )
  }

  const outputPanel = (
    <Panel
      icon={<Terminal />}
      title={outputTitle}
      extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
      actions={
        <>
          {isEscape && (
            <SegmentedControl
              size="sm"
              value={escapeDir}
              onValueChange={setEscapeDir}
              aria-label={t('tabs.escape')}
              options={[
                { value: 'escape', label: t('output.escapeShort') },
                { value: 'unescape', label: t('output.unescapeShort') },
              ]}
            />
          )}
          {!isEscape && tab !== 'validate' && (
            <Tooltip content={t('output.sortKeys')}>
              <Button
                size="icon-sm"
                variant={sortKeys ? 'soft' : 'outline'}
                aria-pressed={sortKeys}
                aria-label={t('output.sortKeys')}
                onClick={() => setSortKeys((v) => !v)}
              >
                <ArrowDownAZ />
              </Button>
            </Tooltip>
          )}
          <Button size="sm" disabled={!result} onClick={copy}>
            <Copy />
            {t('common:copy')}
          </Button>
          <Button size="sm" disabled={!result} onClick={download}>
            <Download />
            {t('common:download')}
          </Button>
          <Tooltip content={t(expanded ? 'output.collapse' : 'output.expand')}>
            <Button
              size="icon-sm"
              aria-label={t(expanded ? 'output.collapse' : 'output.expand')}
              onClick={() => setExpanded((v) => !v)}
            >
              {expanded ? <Minimize2 /> : <Maximize2 />}
            </Button>
          </Tooltip>
        </>
      }
      footer={
        <Footer cursor={result && tab !== 'validate' ? cursorOut : undefined} chars={result?.stats?.chars ?? result?.output.length ?? 0}>
          {result && <span>{t('status.elapsed', { ms: result.elapsedMs })}</span>}
          <WrapToggle checked={wrapOut} onChange={setWrapOut} />
          {!isEscape && (
            <Select<Indent>
              size="sm"
              variant="ghost"
              value={indent}
              onValueChange={setIndent}
              aria-label={t('status.indent')}
              options={[
                { value: '2', label: t('status.indent2') },
                { value: '4', label: t('status.indent4') },
                { value: 'tab', label: t('status.indentTab') },
              ]}
            />
          )}
        </Footer>
      }
    >
      {outputBody()}
    </Panel>
  )

  const sidePanel = (
    <Panel bodyClassName="flex flex-col">
      <div className="shrink-0 border-b border-border px-2 pt-2">
        <Tabs
          variant="underline"
          value={side}
          onValueChange={setSide}
          items={[
            { value: 'tree', label: t('tabs.tree') },
            { value: 'stats', label: t('side.stats') },
          ]}
        />
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {side === 'tree' ? <TreeView docId={error ? null : (result?.docId ?? null)} /> : <StatsView stats={error ? null : (result?.stats ?? null)} />}
      </div>
    </Panel>
  )

  let workspace: ReactNode
  if (expanded && tab !== 'diff' && tab !== 'tree') {
    workspace = <div className="grid h-full grid-cols-1">{outputPanel}</div>
  } else if (tab === 'tree') {
    workspace = (
      <div className="grid h-full grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)] gap-3">
        {inputPanel}
        <Panel icon={<ListTree />} title={t('tabs.tree')} extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}>
          {error ? <ErrorCard error={error} onLocate={(l, c) => inputRef.current?.gotoLine(l, c)} /> : <TreeView docId={result?.docId ?? null} />}
        </Panel>
      </div>
    )
  } else if (tab === 'diff') {
    workspace = (
      <div className="grid h-full grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(280px,0.8fr)] gap-3">
        {inputPanel}
        <Panel
          icon={<FileJson />}
          title={t('input.right')}
          footer={
            <Footer chars={right.length}>
              <WrapToggle checked={wrapOut} onChange={setWrapOut} />
            </Footer>
          }
        >
          <CodeEditor
            value={right}
            onChange={setRight}
            lineWrapping={wrapOut}
            errorLine={rightErrorLine}
            placeholder={t('input.placeholder')}
            aria-label={t('input.right')}
          />
        </Panel>
        <Panel icon={<GitCompare />} title={t('diff.title')} extra={diff.pending && <Spinner className="size-3.5 text-fg-subtle" />}>
          <DiffResult report={diff.result} error={diff.error} pending={diff.pending} />
        </Panel>
      </div>
    )
  } else if (isEscape) {
    workspace = (
      <div className="grid h-full grid-cols-2 gap-3">
        {inputPanel}
        {outputPanel}
      </div>
    )
  } else {
    workspace = (
      <div className="grid h-full grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(240px,0.55fr)] gap-3">
        {inputPanel}
        {outputPanel}
        {sidePanel}
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={(value) => {
          setTab(value)
          setExpanded(false)
        }}
        items={tabs}
        aria-label={t('name')}
      />
      <div className={cn('min-h-0 flex-1')}>{workspace}</div>
    </div>
  )
}
