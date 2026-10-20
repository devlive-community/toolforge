import { useRef, useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, Pagination, Panel, Spinner, toast } from '@toolforge/ui'
import { host, useLaunchInput, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ClipboardPaste, Copy, Download, FileText, FolderOpen, Play, Plus, Search, Square, X } from 'lucide-react'

interface Namespace { prefix: string; uri: string }
interface Item { index: number; kind: string; path: string; value: string; fragment: string | null; valueTruncated: boolean; fragmentTruncated: boolean }
interface Page { handle: number; revision: number; kind: string; scalar: string | null; scalarTruncated: boolean; total: number; offset: number; limit: number; items: Item[]; elapsedMs: number }
const SAMPLE = `<catalog xmlns="urn:toolforge:catalog">
  <item id="one"><name> XML </name><price>12</price></item>
  <item id="two"><name><![CDATA[XPath & text]]></name><price>25</price></item>
</catalog>`
const DEFAULT_NAMESPACES: Namespace[] = [{ prefix: 'c', uri: 'urn:toolforge:catalog' }]

export function XPathQuery() {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<Page>()
  const exporting = useTask<{ bytes: number }>()
  const [input, setInput] = useState(SAMPLE)
  const [expression, setExpression] = usePluginState('xpath-expression', '//c:item')
  const [namespaces, setNamespaces] = usePluginState<Namespace[]>('xpath-namespaces', DEFAULT_NAMESPACES)
  const [revision, setRevision] = useState(0)
  const revisionRef = useRef(0)
  const [startedRevision, setStartedRevision] = useState(-1)
  const [loadedPage, setLoadedPage] = useState<Page | null>(null)
  const [loadingPage, setLoadingPage] = useState(false)
  const busy = task.running || exporting.running
  const result = task.result?.revision === revision && task.status === 'succeeded' ? task.result : null
  const page = loadedPage?.handle === result?.handle ? loadedPage : result

  const invalidate = () => { revisionRef.current += 1; setRevision(revisionRef.current); setLoadedPage(null) }
  const changeInput = (text: string) => { invalidate(); setInput(text) }
  useLaunchInput((text) => changeInput(text))
  const attempt = async (action: () => Promise<unknown>) => {
    try { await action() } catch (reason) { toast.error(errorMessage(reason)) }
  }
  const run = async () => {
    invalidate()
    setStartedRevision(revisionRef.current)
    await task.start('query', { input, expression, namespaces, revision: revisionRef.current })
  }
  const open = async () => {
    const path = await host.dialog.openFile([{ name: t('xpath.xmlFile'), extensions: ['xml', 'svg', 'xhtml', 'xsd', 'xsl', 'txt'] }])
    if (path) changeInput(await host.fs.readText(path))
  }
  const setNamespace = (index: number, key: keyof Namespace, value: string) => {
    invalidate(); setNamespaces(namespaces.map((ns, i) => i === index ? { ...ns, [key]: value } : ns))
  }
  const copy = async (index?: number, fragment = false) => {
    if (!result) return
    const handle = result.handle
    const text = await call<string>('copy', { handle, index, fragment })
    if (revisionRef.current !== result.revision) return
    await host.clipboard.writeText(text)
    toast.success(t('xpath.copied'))
  }
  const save = async () => {
    if (!result) return
    const path = await host.dialog.saveFile('xpath-result.json', [{ name: t('xpath.jsonFile'), extensions: ['json'] }])
    if (path && revisionRef.current === result.revision) await exporting.start('export', { handle: result.handle, path })
  }
  const goToPage = async (number: number) => {
    if (!result) return
    setLoadingPage(true)
    try {
      const next = await call<Page>('page', { handle: result.handle, offset: (number - 1) * 20, limit: 20 })
      if (revisionRef.current === next.revision) setLoadedPage(next)
    } catch (reason) { toast.error(errorMessage(reason)) } finally { setLoadingPage(false) }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="space-y-2 rounded-card border border-border bg-surface p-3">
        <div className="flex gap-2">
          <Input wrapperClassName="min-w-0 flex-1" className="font-mono" value={expression} disabled={busy} aria-label={t('xpath.expression')} placeholder={t('xpath.expressionPlaceholder')} onChange={(event) => { invalidate(); setExpression(event.target.value) }} onKeyDown={(event) => { if (event.key === 'Enter' && !busy) void run() }} />
          <Button disabled={busy || !input.trim() || !expression.trim()} onClick={() => void run()}><Play />{t('xpath.run')}</Button>
          {task.running && <Button variant="secondary" onClick={() => void task.cancel()}><Square />{t('xpath.cancel')}</Button>}
        </div>
        <p className="text-xs text-fg-muted">{t('xpath.namespaceHint')}</p>
        <div className="space-y-1.5">
          {namespaces.map((ns, index) => (
            <div className="flex gap-2" key={index}>
              <Input wrapperClassName="w-28 shrink-0" value={ns.prefix} disabled={busy} aria-label={t('xpath.prefix')} placeholder={t('xpath.prefix')} onChange={(event) => setNamespace(index, 'prefix', event.target.value)} />
              <Input wrapperClassName="min-w-0 flex-1" value={ns.uri} disabled={busy} aria-label={t('xpath.uri')} placeholder={t('xpath.uri')} onChange={(event) => setNamespace(index, 'uri', event.target.value)} />
              <Button size="icon-sm" variant="ghost" disabled={busy} aria-label={t('xpath.removeNamespace')} onClick={() => { invalidate(); setNamespaces(namespaces.filter((_, i) => i !== index)) }}><X /></Button>
            </div>
          ))}
          <Button size="sm" variant="ghost" disabled={busy || namespaces.length >= 32} onClick={() => { invalidate(); setNamespaces([...namespaces, { prefix: '', uri: '' }]) }}><Plus />{t('xpath.addNamespace')}</Button>
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 gap-3 lg:grid-cols-2">
        <Panel title={t('input')} className="min-h-64" actions={<>
          <Button size="icon-sm" variant="ghost" disabled={busy} aria-label={t('xpath.paste')} onClick={() => void attempt(async () => changeInput(await host.clipboard.readText()))}><ClipboardPaste /></Button>
          <Button size="icon-sm" variant="ghost" disabled={busy} aria-label={t('xpath.open')} onClick={() => void attempt(open)}><FolderOpen /></Button>
          <Button size="icon-sm" variant="ghost" disabled={busy} aria-label={t('xpath.sample')} onClick={() => { changeInput(SAMPLE); setExpression('//c:item'); setNamespaces(DEFAULT_NAMESPACES) }}><FileText /></Button>
        </>}>
          <CodeEditor language="xml" value={input} onChange={changeInput} readOnly={busy} className="h-full" />
        </Panel>
        <Panel title={t('xpath.results')} className="min-h-64" bodyClassName="overflow-auto" extra={page ? <Badge variant="neutral">{t(`xpath.types.${page.kind}`)}</Badge> : null} actions={<>
          {page?.scalar !== null && page?.scalar !== undefined && <Button size="icon-sm" variant="ghost" disabled={busy} aria-label={t('xpath.copyValue')} onClick={() => void attempt(() => copy())}><Copy /></Button>}
          <Button size="icon-sm" variant="ghost" disabled={!result || busy} aria-label={t('xpath.export')} onClick={() => void attempt(save)}><Download /></Button>
          {exporting.running && <Button size="icon-sm" variant="ghost" aria-label={t('xpath.cancel')} onClick={() => void exporting.cancel()}><Square /></Button>}
        </>}>
          {task.running ? <div className="flex h-full items-center justify-center gap-2 text-fg-muted"><Spinner />{t('xpath.running')}</div> : task.error && startedRevision === revision ? <p className="m-4 rounded-control bg-danger-soft p-3 text-sm text-danger" role="alert">{errorMessage(task.error)}</p> : !page ? <Empty icon={<Search />} title={t('xpath.empty')} description={t('xpath.emptyHint')} /> : (
            <div className="space-y-3 p-3">
              <p className="text-xs text-fg-muted">{page.kind === 'nodeset' ? t('xpath.count', { count: page.total, ms: page.elapsedMs }) : t('xpath.scalarHint')}</p>
              {page.scalar !== null ? <><pre className="whitespace-pre-wrap break-all rounded-control bg-surface-2 p-3 font-mono text-sm text-fg" data-selectable>{page.scalar}</pre>{page.scalarTruncated && <p className="text-xs text-warning">{t('xpath.truncated')}</p>}</> : page.total === 0 ? <p className="text-sm text-fg-muted">{t('xpath.zero')}</p> : page.items.map((item) => (
                <section className="space-y-2 rounded-control border border-border p-3" key={item.index}>
                  <div className="flex items-center gap-2"><Badge variant="neutral">{t(`xpath.nodes.${item.kind}`)}</Badge><span className="text-xs text-fg-subtle">{item.index + 1}</span><Button className="ml-auto" size="sm" variant="ghost" disabled={busy} onClick={() => void attempt(() => copy(item.index))}><Copy />{t('xpath.copyValue')}</Button></div>
                  <code className="block break-all text-xs text-fg-muted" data-selectable>{item.path}</code>
                  <pre className="whitespace-pre-wrap break-all font-mono text-xs text-fg" data-selectable>{item.value}</pre>
                  {item.valueTruncated && <p className="text-xs text-warning">{t('xpath.truncated')}</p>}
                  {item.fragment !== null && <details><summary className="cursor-pointer text-xs text-fg-muted">{t('xpath.fragment')}</summary><pre className="mt-2 whitespace-pre-wrap break-all rounded-control bg-surface-2 p-2 font-mono text-xs text-fg" data-selectable>{item.fragment}</pre>{item.fragmentTruncated && <p className="text-xs text-warning">{t('xpath.truncated')}</p>}<Button size="sm" variant="ghost" disabled={busy} onClick={() => void attempt(() => copy(item.index, true))}><Copy />{t('xpath.copyFragment')}</Button></details>}
                </section>
              ))}
              {page.kind === 'nodeset' && <Pagination page={Math.floor(page.offset / page.limit) + 1} pageSize={page.limit} total={page.total} onPageChange={(next) => { if (!loadingPage) void goToPage(next) }} prevLabel={t('xpath.prev')} nextLabel={t('xpath.next')} pageLabel={(number) => t('xpath.page', { number })} totalLabel={t('xpath.total', { count: page.total })} />}
              {loadingPage && <Spinner />}
              {exporting.error && <p className="text-xs text-danger" role="alert">{errorMessage(exporting.error)}</p>}
              {exporting.status === 'succeeded' && <p className="text-xs text-success" role="status">{t('xpath.saved')}</p>}
            </div>
          )}
        </Panel>
      </div>
      <p className="text-xs text-fg-subtle">{t('xpath.support')}</p>
    </div>
  )
}
