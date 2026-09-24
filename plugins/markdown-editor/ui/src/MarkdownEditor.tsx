import { useEffect, useEffectEvent, useRef, useState, type KeyboardEvent, type MouseEvent, type ReactNode } from 'react'
import { Button, CodeEditor, DropdownMenu, Empty, Modal, Panel, SegmentedControl, Spinner, Tooltip, cn, toast, type CodeEditorHandle } from '@toolforge/ui'
import { host, useCopy, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import {
  Bold,
  Code,
  Ellipsis,
  FileDown,
  FilePlus,
  FileText,
  FolderOpen,
  FolderSearch,
  Heading,
  Image,
  Italic,
  Link,
  List,
  ListOrdered,
  ListTodo,
  ListTree,
  Minus,
  Save,
  SquareCode,
  Strikethrough,
  Table,
  TextQuote,
} from 'lucide-react'
import type { Draft, Heading as OutlineHeading, Rendered, ViewMode } from './types'

const MARKDOWN_EXTENSIONS = ['md', 'markdown', 'mdown', 'mkd', 'txt']
const HEADING_PREFIX = /^#{1,6}\s+/
const LIST_PREFIX = /^(\s*)([-*+]|\d+\.)\s+(\[[ xX]\]\s+)?/
const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const stem = (path: string) => baseName(path).replace(/\.[^.]+$/, '')
const isMarkdown = (path: string) => MARKDOWN_EXTENSIONS.includes(path.split('.').pop()?.toLowerCase() ?? '')

/** 预览排版：全部使用语义 token，跟随应用主题 */
const PROSE = cn(
  'text-[14px] leading-relaxed text-fg break-words',
  '[&>:first-child]:mt-0 [&_p]:my-3 [&_ul]:my-3 [&_ol]:my-3 [&_table]:my-3 [&_pre]:my-3 [&_blockquote]:my-3',
  '[&_h1]:mt-6 [&_h1]:mb-3 [&_h1]:border-b [&_h1]:border-border [&_h1]:pb-2 [&_h1]:text-2xl [&_h1]:font-semibold',
  '[&_h2]:mt-6 [&_h2]:mb-3 [&_h2]:border-b [&_h2]:border-border [&_h2]:pb-1.5 [&_h2]:text-xl [&_h2]:font-semibold',
  '[&_h3]:mt-5 [&_h3]:mb-2 [&_h3]:text-base [&_h3]:font-semibold [&_h4]:mt-4 [&_h4]:mb-2 [&_h4]:font-semibold',
  '[&_h5]:mt-4 [&_h5]:font-semibold [&_h6]:mt-4 [&_h6]:font-semibold [&_h6]:text-fg-muted',
  '[&_a]:text-primary [&_a]:underline-offset-2 [&_a:hover]:underline',
  '[&_ul]:list-disc [&_ul]:pl-6 [&_ol]:list-decimal [&_ol]:pl-6 [&_li]:my-1 [&_li:has(>input)]:list-none',
  '[&_li>input]:mr-1.5 [&_li>input]:-ml-5 [&_li>input]:align-middle [&_li>input]:accent-primary',
  '[&_blockquote]:border-l-4 [&_blockquote]:border-border-strong [&_blockquote]:pl-3.5 [&_blockquote]:text-fg-muted',
  '[&_.markdown-alert]:border-info [&_.markdown-alert]:text-fg [&_.markdown-alert-tip]:border-success',
  '[&_.markdown-alert-important]:border-primary [&_.markdown-alert-warning]:border-warning [&_.markdown-alert-caution]:border-danger',
  '[&_code]:rounded-sm [&_code]:bg-active [&_code]:px-1 [&_code]:py-px [&_code]:font-mono [&_code]:text-[0.9em]',
  '[&_pre]:overflow-auto [&_pre]:rounded-control [&_pre]:bg-surface-2 [&_pre]:p-3 [&_pre]:text-[12.5px] [&_pre]:leading-normal',
  '[&_pre_code]:bg-transparent [&_pre_code]:p-0',
  '[&_table]:block [&_table]:w-max [&_table]:max-w-full [&_table]:overflow-auto [&_table]:border-collapse',
  '[&_th]:border [&_th]:border-border [&_th]:bg-surface-2 [&_th]:px-3 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-semibold',
  '[&_td]:border [&_td]:border-border [&_td]:px-3 [&_td]:py-1.5',
  '[&_hr]:my-6 [&_hr]:border-border [&_img]:max-w-full [&_img]:rounded-control',
  '[&_.footnote-definition]:flex [&_.footnote-definition]:gap-2 [&_.footnote-definition]:text-xs [&_.footnote-definition]:text-fg-muted',
  '[&_.footnote-definition_p]:my-0 [&_sup]:text-[0.75em]',
)

export function MarkdownEditor() {
  const { t, call, errorMessage } = usePlugin()
  const editor = useRef<CodeEditorHandle>(null)
  const preview = useRef<HTMLDivElement>(null)
  const [draft, setDraft, loaded] = usePluginState<Draft>('draft', { text: t('sample'), path: null, dirty: false })
  const [view, setView] = usePluginState<ViewMode>('view', 'split')
  const [showOutline, setShowOutline] = usePluginState('outline', true)
  const [cursor, setCursor] = useState({ line: 1, column: 1 })
  const [pendingOpen, setPendingOpen] = useState<(() => void) | null>(null)
  const [busy, setBusy] = useState(false)
  const { result } = useDebouncedCall<Rendered>('render', { source: draft.text }, [draft.text])
  const { copy } = useCopy()

  const name = draft.path ? baseName(draft.path) : t('file.untitled')

  const run = async (action: () => Promise<void>) => {
    setBusy(true)
    try {
      await action()
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  /** 有未保存修改时先确认 */
  const guard = (action: () => void) => (draft.dirty ? setPendingOpen(() => action) : action())

  const openPath = (path: string) =>
    run(async () => {
      const text = await host.fs.readText(path)
      setDraft({ text, path, dirty: false })
    })

  const pickFile = () =>
    run(async () => {
      const path = await host.dialog.openFile([{ name: t('file.filter'), extensions: MARKDOWN_EXTENSIONS }])
      if (path) guard(() => openPath(path))
    })

  const saveTo = async (path: string) => {
    await host.fs.writeText(path, draft.text)
    setDraft({ ...draft, path, dirty: false })
    toast.success(t('file.saved', { name: baseName(path) }))
  }

  const saveAs = () =>
    run(async () => {
      const suggested = draft.path ?? `${result?.title ?? t('file.untitled')}.md`
      const path = await host.dialog.saveFile(suggested, [{ name: t('file.filter'), extensions: ['md', 'markdown'] }])
      if (path) await saveTo(path)
    })

  const save = () => (draft.path ? run(() => saveTo(draft.path!)) : saveAs())

  const exportHtml = () =>
    run(async () => {
      const title = draft.path ? stem(draft.path) : undefined
      const path = await host.dialog.saveFile(`${title ?? result?.title ?? t('file.untitled')}.html`, [
        { name: t('file.htmlFilter'), extensions: ['html', 'htm'] },
      ])
      if (!path) return
      const { html } = await call<{ html: string }>('export_html', { source: draft.text, title })
      await host.fs.writeText(path, html)
      toast.success(t('file.exported', { name: baseName(path) }))
    })

  const onDrop = useEffectEvent((paths: string[]) => {
    const path = paths.find(isMarkdown)
    if (path) guard(() => openPath(path))
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: onDrop })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const format = {
    heading: () => editor.current?.toggleLinePrefix('## ', HEADING_PREFIX),
    bold: () => editor.current?.wrapSelection('**'),
    italic: () => editor.current?.wrapSelection('*'),
    strike: () => editor.current?.wrapSelection('~~'),
    code: () => editor.current?.wrapSelection('`'),
    link: () => editor.current?.wrapSelection('[', '](https://)'),
    image: () => editor.current?.insertText(`![${t('format.imageAlt')}](https://)`),
    quote: () => editor.current?.toggleLinePrefix('> '),
    bullet: () => editor.current?.toggleLinePrefix('- ', LIST_PREFIX),
    ordered: () => editor.current?.toggleLinePrefix('1. ', LIST_PREFIX),
    task: () => editor.current?.toggleLinePrefix('- [ ] ', LIST_PREFIX),
    codeBlock: () => editor.current?.wrapSelection('```\n', '\n```'),
    table: () => editor.current?.insertText(`\n${t('format.tableHead')}\n`),
    rule: () => editor.current?.insertText('\n---\n'),
  }

  const onKeyDown = (event: KeyboardEvent) => {
    if (!(event.metaKey || event.ctrlKey) || event.altKey) return
    const key = event.key.toLowerCase()
    const action =
      key === 's' ? (event.shiftKey ? saveAs : save) : event.shiftKey ? undefined : { b: format.bold, i: format.italic, k: format.link }[key]
    if (!action) return
    event.preventDefault()
    event.stopPropagation()
    action()
  }

  const scrollPreviewTo = (id: string) => {
    const target = preview.current?.querySelector(`[id="${CSS.escape(id)}"]`)
    target?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }

  const jump = (heading: OutlineHeading) => {
    if (view !== 'preview') editor.current?.gotoLine(heading.line)
    if (view !== 'edit') scrollPreviewTo(heading.id)
  }

  /** 预览中的链接：页内锚点滚动，网页链接用系统浏览器打开 */
  const onPreviewClick = (event: MouseEvent) => {
    const anchor = (event.target as HTMLElement).closest('a')
    if (!anchor) return
    event.preventDefault()
    const href = anchor.getAttribute('href') ?? ''
    if (href.startsWith('#')) scrollPreviewTo(decodeURIComponent(href.slice(1)))
    else if (/^https?:\/\//i.test(href)) host.openUrl(href).catch((error) => toast.error(errorMessage(error)))
  }

  const stats = result?.stats
  const outline = result?.outline ?? []

  return (
    <div className="flex h-full min-h-0 flex-col gap-3" onKeyDownCapture={onKeyDown}>
      <div className="flex flex-wrap items-center gap-1.5">
        <Button size="sm" onClick={() => guard(() => setDraft({ text: '', path: null, dirty: false }))} disabled={busy}>
          <FilePlus />
          {t('file.new')}
        </Button>
        <Button size="sm" onClick={pickFile} disabled={busy}>
          <FolderOpen />
          {t('file.open')}
        </Button>
        <Button size="sm" variant="primary" onClick={save} disabled={busy}>
          {busy ? <Spinner /> : <Save />}
          {t('file.save')}
        </Button>
        <DropdownMenu
          trigger={
            <Button size="icon-sm" variant="ghost" aria-label={t('file.more')}>
              <Ellipsis />
            </Button>
          }
          placement="bottom-start"
          items={[
            { key: 'saveAs', label: t('file.saveAs'), icon: <Save />, onSelect: saveAs },
            { key: 'export', label: t('file.export'), icon: <FileDown />, onSelect: exportHtml },
            { key: 'copy', label: t('file.copyHtml'), icon: <Code />, disabled: !result, onSelect: () => result && copy(result.html) },
            ...(draft.path
              ? [{ key: 'reveal', label: t('file.reveal'), icon: <FolderSearch />, onSelect: () => host.revealPath(draft.path!).catch((e) => toast.error(errorMessage(e))) }]
              : []),
          ]}
        />
        <span className="mx-1 h-5 w-px bg-border" />
        <div className={cn('flex flex-wrap items-center gap-0.5', view === 'preview' && 'pointer-events-none opacity-40')}>
          <Tool label={t('format.heading')} onClick={format.heading} icon={<Heading />} />
          <Tool label={t('format.bold')} onClick={format.bold} icon={<Bold />} />
          <Tool label={t('format.italic')} onClick={format.italic} icon={<Italic />} />
          <Tool label={t('format.strike')} onClick={format.strike} icon={<Strikethrough />} />
          <Tool label={t('format.code')} onClick={format.code} icon={<Code />} />
          <Tool label={t('format.link')} onClick={format.link} icon={<Link />} />
          <Tool label={t('format.image')} onClick={format.image} icon={<Image />} />
          <Tool label={t('format.quote')} onClick={format.quote} icon={<TextQuote />} />
          <Tool label={t('format.bullet')} onClick={format.bullet} icon={<List />} />
          <Tool label={t('format.ordered')} onClick={format.ordered} icon={<ListOrdered />} />
          <Tool label={t('format.task')} onClick={format.task} icon={<ListTodo />} />
          <Tool label={t('format.codeBlock')} onClick={format.codeBlock} icon={<SquareCode />} />
          <Tool label={t('format.table')} onClick={format.table} icon={<Table />} />
          <Tool label={t('format.rule')} onClick={format.rule} icon={<Minus />} />
        </div>
        <div className="ml-auto flex items-center gap-1.5">
          <Tooltip content={t('view.outline')}>
            <Button
              size="icon-sm"
              variant={showOutline ? 'secondary' : 'ghost'}
              aria-label={t('view.outline')}
              aria-pressed={showOutline}
              onClick={() => setShowOutline(!showOutline)}
            >
              <ListTree />
            </Button>
          </Tooltip>
          <SegmentedControl<ViewMode>
            size="sm"
            value={view}
            onValueChange={setView}
            aria-label={t('view.label')}
            options={[
              { value: 'edit', label: t('view.edit') },
              { value: 'split', label: t('view.split') },
              { value: 'preview', label: t('view.preview') },
            ]}
          />
        </div>
      </div>

      <div className={cn('grid min-h-0 flex-1 gap-3', showOutline ? 'grid-cols-[200px_minmax(0,1fr)]' : 'grid-cols-1')}>
        {showOutline && (
          <Panel icon={<ListTree />} title={t('outline.title')} bodyClassName="overflow-auto p-1.5">
            {outline.length === 0 ? (
              <p className="p-2 text-xs text-fg-subtle">{t('outline.empty')}</p>
            ) : (
              <ul>
                {outline.map((heading) => (
                  <li key={heading.id}>
                    <button
                      type="button"
                      onClick={() => jump(heading)}
                      style={{ paddingLeft: `${(heading.level - 1) * 12 + 8}px` }}
                      className={cn(
                        'block w-full truncate rounded-control py-1 pr-2 text-left text-[12.5px] outline-none hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring',
                        heading.level === 1 ? 'font-semibold text-fg' : 'text-fg-muted',
                      )}
                      title={heading.text}
                    >
                      {heading.text}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Panel>
        )}

        <div className={cn('grid min-h-0 gap-3', view === 'split' ? 'grid-cols-2' : 'grid-cols-1')}>
          {view !== 'preview' && (
            <Panel
              icon={<FileText />}
              title={
                <span className="flex items-center gap-1.5">
                  {name}
                  {draft.dirty && (
                    <Tooltip content={t('file.unsaved')}>
                      <span className="size-1.5 rounded-full bg-warning" aria-label={t('file.unsaved')} />
                    </Tooltip>
                  )}
                </span>
              }
              bodyClassName="p-0"
            >
              {loaded ? (
                <CodeEditor
                  ref={editor}
                  value={draft.text}
                  onChange={(text) => setDraft({ ...draft, text, dirty: true })}
                  onCursorChange={setCursor}
                  language="markdown"
                  lineWrapping
                  aria-label={name}
                />
              ) : (
                <div className="flex h-full items-center justify-center">
                  <Spinner />
                </div>
              )}
            </Panel>
          )}
          {view !== 'edit' && (
            <Panel icon={<FileText />} title={t('view.preview')} bodyClassName="overflow-auto">
              {result && result.html ? (
                <div ref={preview} onClick={onPreviewClick} className={cn(PROSE, 'px-6 py-5')} data-selectable dangerouslySetInnerHTML={{ __html: result.html }} />
              ) : (
                <Empty icon={<FileText />} title={t('preview.empty')} />
              )}
            </Panel>
          )}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-1 text-xs text-fg-muted tabular-nums">
        {stats && (
          <>
            <span>{t('stats.words', { count: stats.words })}</span>
            <span>{t('stats.chars', { count: stats.chars })}</span>
            <span>{t('stats.lines', { count: stats.lines })}</span>
            {stats.readingMinutes > 0 && <span>{t('stats.reading', { count: stats.readingMinutes })}</span>}
            {stats.tasks > 0 && <span>{t('stats.tasks', { done: stats.tasksDone, total: stats.tasks })}</span>}
          </>
        )}
        {view !== 'preview' && <span className="ml-auto">{t('stats.cursor', cursor)}</span>}
      </div>

      <Modal
        open={pendingOpen !== null}
        onOpenChange={(open) => !open && setPendingOpen(null)}
        size="sm"
        title={t('discard.title')}
        description={t('discard.description', { name })}
        footer={
          <>
            <Button onClick={() => setPendingOpen(null)}>{t('discard.cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                pendingOpen?.()
                setPendingOpen(null)
              }}
            >
              {t('discard.confirm')}
            </Button>
          </>
        }
      />
    </div>
  )
}

function Tool({ label, icon, onClick }: { label: string; icon: ReactNode; onClick: () => void }) {
  return (
    <Tooltip content={label}>
      <Button size="icon-sm" variant="ghost" aria-label={label} onClick={onClick}>
        {icon}
      </Button>
    </Tooltip>
  )
}
