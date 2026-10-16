import { useEffect, useState } from 'react'
import {
  Badge, Button, Checkbox, CodeEditor, Drawer, Empty, Input, Modal, Panel, Select, Spinner, Tabs, cn, toast,
} from '@toolforge/ui'
import { host, useCopy, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import {
  ArrowDown, ArrowUp, Blocks, Braces, Check, Copy, Download, ExternalLink, FileCode2, FilePlus2,
  FileX2, Info, Layers3, Monitor, Pencil, Plus, Search, Settings2, Trash2, X,
} from 'lucide-react'
import { createExportSession, saveCurrentOutput } from './exportSession'
import { useLibrary } from './useLibrary'
import type { Catalog, Category, CustomTemplate, Generated, InputTab, LineEnding, Template } from './types'

const CATEGORIES: Category[] = ['all', 'language', 'framework', 'editor', 'os', 'custom']
const CATEGORY_ICONS = { language: Braces, framework: Blocks, editor: FileCode2, os: Monitor, custom: FilePlus2 }

function TemplateRow({ template, selected, onToggle, onEdit, onDelete, disabled }: {
  template: Template
  selected: boolean
  onToggle: () => void
  onEdit?: () => void
  onDelete?: () => void
  disabled: boolean
}) {
  const { t } = usePlugin()
  const Icon = CATEGORY_ICONS[template.category]
  return (
    <div className={cn(
      'flex items-center gap-1 rounded-control border transition-colors',
      selected ? 'border-primary/40 bg-primary-soft/40' : 'border-border hover:bg-hover',
    )}>
      <Checkbox checked={selected} onCheckedChange={onToggle} disabled={disabled} aria-label={t('catalog.toggle', { name: template.name })} className="min-w-0 flex-1 p-2.5 text-left">
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 text-[13px] font-medium">
          <Icon className="size-3.5 shrink-0 text-fg-subtle" />
          <span className="truncate">{template.name}</span>
        </span>
        <span className="mt-0.5 block text-[11px] text-fg-subtle">
          {t('category.' + template.category)}
          <span className="mx-1.5">·</span>
          {t('catalog.rules', { count: template.ruleCount })}
        </span>
      </span>
      </Checkbox>
      {template.category === 'custom' && (
        <div className="flex shrink-0 items-center pr-1">
          <Button variant="ghost" size="icon-sm" disabled={disabled} onClick={onEdit} aria-label={t('library.editName', { name: template.name })} title={t('library.edit')}><Pencil /></Button>
          <Button variant="ghost" size="icon-sm" disabled={disabled} onClick={onDelete} aria-label={t('library.deleteName', { name: template.name })} title={t('library.delete')}><Trash2 /></Button>
        </div>
      )}
    </div>
  )
}

function GitignoreTool() {
  const { t, errorMessage } = usePlugin()
  const { copy, copied } = useCopy()
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState<Category>('all')
  const [templateIds, setTemplateIds] = useState<string[]>([])
  const [baseContent, setBaseContent] = useState('')
  const [customRules, setCustomRules] = useState('')
  const [lineEnding, setLineEnding] = useState<LineEnding>('lf')
  const [inputTab, setInputTab] = useState<InputTab>('templates')
  const [sourceOpen, setSourceOpen] = useState(false)
  const [saving, setSaving] = useState(false)
  const [exports] = useState(createExportSession)
  const library = useLibrary()
  const [draft, setDraft] = useState<{ id?: string; name: string; content: string } | null>(null)
  const [deleting, setDeleting] = useState<CustomTemplate | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)

  useEffect(() => {
    exports.activate()
    return () => exports.close()
  }, [exports])

  const catalog = useDebouncedCall<Catalog>('catalog', { library: library.library }, [library.library])
  const filtering = query.length > 0 || category !== 'all'
  const filtered = useDebouncedCall<Catalog>(filtering ? 'catalog' : null, { query, category, library: library.library }, [query, category, library.library])
  const listing = filtering ? filtered : catalog
  const visibleTemplates = listing.pending ? [] : listing.result?.templates ?? []
  const templates = new Map(catalog.result?.templates.map((template) => [template.id, template]))
  const generated = useDebouncedCall<Generated>(
    'generate',
    { templateIds, baseContent, customRules, lineEnding, library: library.library },
    [templateIds, baseContent, customRules, lineEnding, library.library, library.pending],
  )
  const result = !generated.pending && !generated.error && !library.pending ? generated.result : null
  const ready = !!result?.output
  const isCurrent = exports.capture()
  const source = catalog.result?.source
  const canManage = !!library.library && !library.loading && !library.error && !library.pending

  const editTemplate = (id?: string) => {
    if (!canManage) return
    const existing = id ? library.library?.templates.find((template) => template.id === id) : undefined
    if (id && !existing) return
    setMutationError(null)
    setDraft(existing ? { ...existing } : { name: '', content: '' })
  }
  const deleteTemplate = (id: string) => {
    if (!canManage) return
    const existing = library.library?.templates.find((template) => template.id === id)
    if (!existing) return
    setMutationError(null)
    setDeleting(existing)
  }
  const saveTemplate = async () => {
    if (!canManage || !draft) return
    exports.invalidate()
    setMutationError(null)
    try {
      const saved = await library.mutate({ operation: 'save', ...draft })
      if (!saved) return
      const id = saved.templateId
      if (id && !draft.id) {
        setTemplateIds((current) => current.includes(id) ? current : [...current, id])
      }
      setDraft(null)
      toast.success(t('library.saved'))
    } catch (reason) {
      setMutationError(errorMessage(reason))
    }
  }
  const confirmDelete = async () => {
    if (!canManage || !deleting) return
    exports.invalidate()
    setMutationError(null)
    try {
      const saved = await library.mutate({ operation: 'delete', id: deleting.id })
      if (!saved) return
      setTemplateIds((current) => current.filter((id) => id !== deleting.id))
      setDeleting(null)
      toast.success(t('library.deleted'))
    } catch (reason) {
      setMutationError(errorMessage(reason))
    }
  }

  const changeTemplates = (next: string[]) => {
    exports.invalidate()
    setTemplateIds(next)
  }
  const toggle = (id: string) => changeTemplates(templateIds.includes(id) ? templateIds.filter((selected) => selected !== id) : [...templateIds, id])
  const move = (index: number, direction: -1 | 1) => {
    const target = index + direction
    if (target < 0 || target >= templateIds.length) return
    const next = [...templateIds]
    ;[next[index], next[target]] = [next[target], next[index]]
    changeTemplates(next)
  }
  const editBase = (value: string) => {
    if (value === baseContent) return
    exports.invalidate()
    setBaseContent(value)
  }
  const editCustom = (value: string) => {
    if (value === customRules) return
    exports.invalidate()
    setCustomRules(value)
  }
  const changeLineEnding = (value: LineEnding) => {
    if (value === lineEnding) return
    exports.invalidate()
    setLineEnding(value)
  }

  const save = async () => {
    if (!ready || !result || saving || !isCurrent()) return
    setSaving(true)
    try {
      const outcome = await saveCurrentOutput(result.output, isCurrent, () => host.dialog.saveFile('.gitignore'), host.fs.writeText)
      if (outcome === 'saved') toast.success(t('common:saved'))
      if (outcome === 'changed') toast.info(t('output.changed'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    } finally {
      setSaving(false)
    }
  }

  const sectionName = (id: string, name: string) => id === 'base' ? t('input.base') : id === 'custom' ? t('input.custom') : name

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(220px,0.7fr)_minmax(0,1.7fr)] gap-3">
        <Panel
          title={t('catalog.title')}
          icon={<Layers3 />}
          extra={catalog.result && <Badge>{catalog.result.templates.length}</Badge>}
          footer={
            <Button variant="ghost" size="sm" className="-ml-1.5 w-full justify-start" onClick={() => setSourceOpen(true)} disabled={!source}>
              <Info />{t('source.button')}
            </Button>
          }
          bodyClassName="flex flex-col"
        >
          <div className="shrink-0 space-y-2 border-b border-border p-2.5">
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t('catalog.search')}
              aria-label={t('catalog.search')}
              leading={<Search />}
              wrapperClassName="w-full"
              spellCheck={false}
            />
            <Select<Category>
              value={category}
              onValueChange={setCategory}
              options={CATEGORIES.map((value) => ({ value, label: t('category.' + value) }))}
              aria-label={t('catalog.category')}
              className="w-full"
              size="sm"
            />
            <Button variant="soft" size="sm" block disabled={!canManage} onClick={() => editTemplate()}>
              {library.loading ? <Spinner /> : <Plus />}{t('library.create')}
            </Button>
            {library.error && (
              <div role="alert" className="space-y-1 text-xs text-danger">
                <p>{t('library.loadFailed')}</p>
                <p className="break-words text-[11px]">{errorMessage(library.error)}</p>
                <Button variant="ghost" size="sm" onClick={library.retry}>{t('library.retry')}</Button>
              </div>
            )}
          </div>
          <div className="min-h-0 flex-1 space-y-2 overflow-auto p-2.5">
            {listing.pending ? (
              <Empty icon={<Spinner />} title={t('catalog.loading')} />
            ) : listing.error ? (
              <p role="alert" className="p-3 text-xs text-danger">{errorMessage(listing.error)}</p>
            ) : visibleTemplates.length === 0 ? (
              <Empty icon={<Search />} title={t('catalog.empty')} description={t('catalog.emptyHint')} />
            ) : visibleTemplates.map((template) => (
              <TemplateRow
                key={template.id}
                template={template}
                selected={templateIds.includes(template.id)}
                onToggle={() => toggle(template.id)}
                onEdit={() => editTemplate(template.id)}
                onDelete={() => deleteTemplate(template.id)}
                disabled={library.pending || (template.category === 'custom' && !canManage)}
              />
            ))}
          </div>
        </Panel>

        <div className="flex min-h-0 min-w-0 flex-col gap-3">
          <Panel
            title={t('input.title')}
            icon={<Settings2 />}
            className="shrink-0"
            actions={
              templateIds.length > 0 && inputTab === 'templates' && (
                <Button variant="ghost" size="sm" onClick={() => changeTemplates([])}>{t('input.clear')}</Button>
              )
            }
            bodyClassName="flex flex-col"
          >
            <Tabs<InputTab>
              value={inputTab}
              onValueChange={setInputTab}
              variant="underline"
              aria-label={t('input.title')}
              items={[
                { value: 'templates', label: t('input.templates', { count: templateIds.length }), icon: <Layers3 /> },
                { value: 'base', label: t('input.base'), icon: <FileCode2 /> },
                { value: 'custom', label: t('input.custom'), icon: <FilePlus2 /> },
              ]}
            />
            {inputTab === 'templates' ? (
              <div className="max-h-40 overflow-auto p-2.5">
                {templateIds.length === 0 ? (
                  <div className="flex items-center gap-2 rounded-control border border-dashed border-border px-3 py-4 text-xs text-fg-subtle">
                    <Layers3 className="size-4 shrink-0" />{t('input.choose')}
                  </div>
                ) : (
                  <>
                    <p className="mb-2 text-[11px] text-fg-subtle">{t('input.orderHint')}</p>
                    <ol className="space-y-1">
                      {templateIds.map((id, index) => (
                        <li key={id} className="flex items-center gap-2 rounded-control bg-surface-2 px-2 py-1">
                          <span className="w-4 shrink-0 text-center text-[11px] text-fg-subtle tabular-nums">{index + 1}</span>
                          <span className="min-w-0 flex-1 truncate text-xs text-fg">{templates.get(id)?.name ?? id}</span>
                          <Button variant="ghost" size="icon-sm" disabled={index === 0} aria-label={t('input.moveUp', { name: templates.get(id)?.name ?? id })} onClick={() => move(index, -1)}><ArrowUp /></Button>
                          <Button variant="ghost" size="icon-sm" disabled={index === templateIds.length - 1} aria-label={t('input.moveDown', { name: templates.get(id)?.name ?? id })} onClick={() => move(index, 1)}><ArrowDown /></Button>
                          <Button variant="ghost" size="icon-sm" aria-label={t('input.remove', { name: templates.get(id)?.name ?? id })} onClick={() => toggle(id)}><X /></Button>
                        </li>
                      ))}
                    </ol>
                  </>
                )}
              </div>
            ) : (
              <>
                <div className="h-36 min-h-0 border-t border-border">
                  <CodeEditor
                    value={inputTab === 'base' ? baseContent : customRules}
                    onChange={inputTab === 'base' ? editBase : editCustom}
                    language="text"
                    placeholder={t(inputTab === 'base' ? 'input.basePlaceholder' : 'input.customPlaceholder')}
                    aria-label={t(inputTab === 'base' ? 'input.base' : 'input.custom')}
                  />
                </div>
                <p className="border-t border-border px-3 py-2 text-[11px] text-fg-subtle">{t(inputTab === 'base' ? 'input.baseHint' : 'input.customHint')}</p>
              </>
            )}
          </Panel>

          <Panel
            title={t('output.title')}
            icon={<FileX2 />}
            className="min-h-40 flex-1"
            extra={generated.pending && <Spinner className="size-3.5 text-fg-subtle" />}
            actions={
              <>
                <Select<LineEnding>
                  value={lineEnding}
                  onValueChange={changeLineEnding}
                  options={[{ value: 'lf', label: t('output.lf') }, { value: 'crlf', label: t('output.crlf') }]}
                  aria-label={t('output.lineEnding')}
                  size="sm"
                  variant="ghost"
                />
                <Button
                  variant="outline"
                  size="icon-sm"
                  aria-label={t('output.copy')}
                  title={t('output.copy')}
                  disabled={!ready}
                  onClick={() => { if (ready && result && isCurrent()) void copy(result.output) }}
                >
                  {copied ? <Check className="text-success" /> : <Copy />}
                </Button>
                <Button variant="primary" size="sm" disabled={!ready} loading={saving} onClick={() => void save()}><Download />{t('output.save')}</Button>
              </>
            }
            footer={result ? (
              <div className="flex min-w-0 flex-1 items-center gap-3 overflow-hidden whitespace-nowrap text-[11px]" aria-live="polite">
                <span>{t('output.rules', { count: result.stats.ruleCount })}</span>
                <span>{t('output.deduplicated', { count: result.stats.duplicateCount })}</span>
                <span className="ml-auto text-fg-subtle">{t('output.lines', { count: result.stats.lineCount })}</span>
              </div>
            ) : <span>{generated.pending ? t('output.updating') : t('output.footer')}</span>}
            bodyClassName="flex flex-col"
          >
            {!generated.pending && generated.error ? (
              <div role="alert" className="p-4 text-xs text-danger">{errorMessage(generated.error)}</div>
            ) : !ready ? (
              <Empty icon={generated.pending ? <Spinner /> : <FileX2 />} title={generated.pending ? t('output.updating') : t('output.empty')} description={generated.pending ? undefined : t('output.emptyHint')} />
            ) : (
              <>
                <div className="flex max-h-20 shrink-0 flex-wrap items-center gap-1.5 overflow-auto border-b border-border bg-surface-2/40 px-3 py-2">
                  {result.sections.map((section) => (
                    <Badge key={section.id} title={t('catalog.rules', { count: section.ruleCount })}>{sectionName(section.id, section.name)}</Badge>
                  ))}
                </div>
                <div className="min-h-0 flex-1"><CodeEditor value={result.output} readOnly language="text" aria-label={t('output.title')} /></div>
              </>
            )}
          </Panel>
        </div>
      </div>

      <Modal
        open={draft !== null}
        onOpenChange={(open) => { if (!open && !library.pending) { setDraft(null); setMutationError(null) } }}
        title={t(draft?.id ? 'library.edit' : 'library.createTitle')}
        size="lg"
        hideClose={library.pending}
        closeLabel={t('library.cancel')}
        footer={
          <>
            <Button disabled={library.pending} onClick={() => { setDraft(null); setMutationError(null) }}>{t('library.cancel')}</Button>
            <Button variant="primary" disabled={!canManage} loading={library.pending} onClick={() => void saveTemplate()}>{t('library.save')}</Button>
          </>
        }
      >
        {draft && (
          <div className="space-y-3 px-5 pb-4">
            <label className="block space-y-1.5 text-xs text-fg-muted">
              <span>{t('library.name')}</span>
              <Input
                autoFocus
                value={draft.name}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
                disabled={library.pending}
                placeholder={t('library.namePlaceholder')}
                aria-label={t('library.name')}
                wrapperClassName="w-full"
              />
            </label>
            <div className="space-y-1.5">
              <p className="text-xs text-fg-muted">{t('library.content')}</p>
              <div className="h-64 overflow-hidden rounded-control border border-border bg-surface">
                <CodeEditor
                  value={draft.content}
                  onChange={(content) => setDraft({ ...draft, content })}
                  readOnly={library.pending}
                  language="text"
                  placeholder={t('library.contentPlaceholder')}
                  aria-label={t('library.content')}
                />
              </div>
            </div>
            {mutationError && <p role="alert" className="text-xs text-danger">{mutationError}</p>}
          </div>
        )}
      </Modal>

      <Modal
        open={deleting !== null}
        onOpenChange={(open) => { if (!open && !library.pending) { setDeleting(null); setMutationError(null) } }}
        title={t('library.delete')}
        description={t('library.deleteConfirm', { name: deleting?.name ?? '' })}
        hideClose={library.pending}
        closeLabel={t('library.cancel')}
        size="sm"
        footer={
          <>
            <Button disabled={library.pending} onClick={() => { setDeleting(null); setMutationError(null) }}>{t('library.cancel')}</Button>
            <Button variant="danger" disabled={!canManage} loading={library.pending} onClick={() => void confirmDelete()}>{t('library.delete')}</Button>
          </>
        }
      >
        {mutationError && <p role="alert" className="px-5 pb-4 text-xs text-danger">{mutationError}</p>}
      </Modal>

      <Drawer open={sourceOpen} onOpenChange={setSourceOpen} title={t('source.title')} closeLabel={t('source.close')}>
        {source && (
          <div className="space-y-5 p-5 text-xs text-fg-muted">
            <p className="leading-relaxed">{t('source.description')}</p>
            <div className="space-y-2 rounded-card border border-border bg-surface p-3">
              <p className="font-medium text-fg">{t('source.repository')}</p>
              <Button variant="ghost" size="sm" className="-ml-2" onClick={() => void host.openUrl(source.url).catch((reason) => toast.error(errorMessage(reason)))}>{t('source.open')}<ExternalLink /></Button>
              <p className="pt-1 text-fg-subtle">{t('source.revision')}</p>
              <code className="block break-all font-mono text-[11px] text-fg" data-selectable>{source.revision}</code>
            </div>
            <div>
              <h3 className="mb-2 font-medium text-fg">{source.license}</h3>
              <pre className="whitespace-pre-wrap break-words font-sans text-[11px] leading-relaxed" data-selectable>{source.licenseText}</pre>
            </div>
          </div>
        )}
      </Drawer>
    </div>
  )
}

export default GitignoreTool
