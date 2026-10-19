import { useRef, useState } from 'react'
import { Badge, Button, Empty, Input, Panel, SegmentedControl, Spinner, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { Binary, FileUp, ListChecks, Play, Square, X } from 'lucide-react'
import type { Diag, Exported, Node, Report } from './types'

const LEVEL_VARIANT = { error: 'danger', warning: 'warning', hint: 'info' } as const
const OVERALL_VARIANT = { passed: 'success', issues: 'warning', incomplete: 'neutral' } as const

type Source = 'file' | 'hex' | 'base64'

function TreeNode({
  node,
  path,
  selected,
  onSelect,
  expanded,
  toggle,
}: {
  node: Node
  path: string
  selected: Node | null
  onSelect: (node: Node) => void
  expanded: Set<string>
  toggle: (path: string) => void
}) {
  const { t } = usePlugin()
  const isBranch = (node.children?.length ?? 0) > 0 || (node.entries?.length ?? 0) > 0
  const open = expanded.has(path) || !isBranch
  const isSelected = selected !== null && selected.offset === node.offset && selected.length === node.length
  const label =
    node.kind === 'tag' && node.number !== undefined && node.number !== null
      ? t('result.tagNumber', { number: node.number })
      : node.kind === 'ext' && node.number !== undefined && node.number !== null
        ? t('result.extNumber', { number: node.number })
        : node.kind
  const preview = node.value ?? null
  const count = node.children?.length ?? node.entries?.length ?? 0
  return (
    <div>
      <button
        type="button"
        onClick={() => {
          onSelect(node)
          if (isBranch) toggle(path)
        }}
        className={`flex w-full items-center gap-1.5 rounded-control px-1 py-0.5 text-left font-mono text-xs ${isSelected ? 'bg-primary-soft' : 'hover:bg-active'}`}
      >
        <span className="w-3 shrink-0 text-fg-subtle">{isBranch ? (open ? '▾' : '▸') : ''}</span>
        <Badge variant="neutral">{label}</Badge>
        <span className="shrink-0 text-fg-subtle">{t('result.offsetAt', { offset: node.offset, length: node.length })}</span>
        {preview !== null && <span className="min-w-0 flex-1 truncate text-fg" data-selectable>{preview}</span>}
        {node.indefinite && <Badge variant="info">{t('result.indefinite')}</Badge>}
        {isBranch && open && (
          <span className="ml-auto shrink-0 text-fg-subtle">
            {t(node.entries ? 'result.entries' : 'result.children', { count })}
          </span>
        )}
      </button>
      {open && isBranch && (
        <div className="ml-4 border-l border-border pl-2">
          {node.children?.map((child, index) => (
            <TreeNode
              key={index}
              node={child}
              path={`${path}/${index}`}
              selected={selected}
              onSelect={onSelect}
              expanded={expanded}
              toggle={toggle}
            />
          ))}
          {node.entries?.map((entry, index) => (
            <div key={index}>
              <div className="pl-1 text-[10px] text-fg-subtle">{t('result.keyLabel')}</div>
              <TreeNode
                node={entry.key}
                path={`${path}/${index}/k`}
                selected={selected}
                onSelect={onSelect}
                expanded={expanded}
                toggle={toggle}
              />
              <div className="pl-1 text-[10px] text-fg-subtle">{t('result.valueLabel')}</div>
              <TreeNode
                node={entry.value}
                path={`${path}/${index}/v`}
                selected={selected}
                onSelect={onSelect}
                expanded={expanded}
                toggle={toggle}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

export default function BinaryInspector() {
  const { t, manifest, errorMessage } = usePlugin()
  const [format, setFormat] = useState<'cbor' | 'msgpack'>('cbor')
  const [source, setSource] = useState<Source>('hex')
  const [text, setText] = useState('')
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const [selected, setSelected] = useState<Node | null>(null)
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(['/']))
  const [cancelRequested, setCancelRequested] = useState(false)
  const task = useTask<Report>()
  const fileSequence = useRef(0)

  const report = task.result

  const toggle = (path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const openFile = async () => {
    const sequence = ++fileSequence.current
    try {
      // 文件内容不经过前端：路径直接交给后端通过宿主读取
      const path = await host.dialog.openFile([
        { name: t('input.fileFilter'), extensions: ['bin', 'cbor', 'mpack', 'dat'] },
      ])
      if (sequence !== fileSequence.current) return
      setSource('file')
      setPendingPath(path)
      if (path) setText('')
    } catch (reason) {
      if (sequence === fileSequence.current) toast.error(errorMessage(reason))
    }
  }

  const start = () => {
    setCancelRequested(false)
    setSelected(null)
    const args = source === 'file' ? { format, path: pendingPath } : { format, encoding: source, text }
    void task.start('inspect', args)
  }
  const cancel = async () => {
    setCancelRequested(true)
    try {
      await task.cancel()
    } catch (reason) {
      setCancelRequested(false)
      toast.error(errorMessage(reason))
    }
  }

  const exportReport = async (kind: 'markdown' | 'json', save: boolean) => {
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format: kind })
      if (save) {
        const path = await host.dialog.saveFile(`binary-report.${out.ext}`, [
          { name: out.ext.toUpperCase(), extensions: [out.ext] },
        ])
        if (!path) return
        await host.fs.writeText(path, out.content)
        toast.success(t('common:saved'))
      } else {
        await host.clipboard.writeText(out.content)
        toast.success(t('common:saved'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(380px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <Panel
        icon={<Binary />}
        title={t('input.source')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        footer={
          task.running ? (
            <div className="flex w-full items-center gap-3">
              <Button size="sm" onClick={cancel} disabled={cancelRequested}>
                <Square />
                {t(cancelRequested ? 'input.cancelling' : 'input.cancel')}
              </Button>
              <span role="status" className="flex items-center gap-2 text-xs text-fg-muted">
                <Spinner className="size-3.5" />
                {task.stage ? t(`input.stages.${task.stage}`) : t('input.run')}
              </span>
            </div>
          ) : (
            <Button
              size="sm"
              variant="primary"
              onClick={start}
              disabled={source === 'file' ? !pendingPath : !text.trim()}
            >
              <Play />
              {t('input.run')}
            </Button>
          )
        }
      >
        <label className="block space-y-1 text-xs text-fg-muted">
          <span>{t('input.format')}</span>
          <SegmentedControl<'cbor' | 'msgpack'>
            value={format}
            onValueChange={setFormat}
            options={[
              { value: 'cbor', label: t('input.format.cbor') },
              { value: 'msgpack', label: t('input.format.msgpack') },
            ]}
          />
        </label>
        <label className="block space-y-1 text-xs text-fg-muted">
          <span>{t('input.source')}</span>
          <SegmentedControl<Source>
            value={source}
            onValueChange={setSource}
            options={[
              { value: 'file', label: t('input.source.file') },
              { value: 'hex', label: t('input.source.hex') },
              { value: 'base64', label: t('input.source.base64') },
            ]}
          />
        </label>
        {source === 'file' ? (
          <div className="flex items-center gap-2">
            <Button onClick={() => void openFile()}>
              <FileUp />
              {t('input.open')}
            </Button>
            {pendingPath && <code className="min-w-0 flex-1 truncate font-mono text-xs text-fg">{pendingPath}</code>}
          </div>
        ) : (
          <Input
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder={t('input.textPlaceholder')}
            aria-label={t('input.source')}
            spellCheck={false}
          />
        )}
        {source !== 'file' && (
          <Tooltip content={t('input.clear')}>
            <Button size="icon-sm" variant="ghost" aria-label={t('input.clear')} onClick={() => setText('')}>
              <X />
            </Button>
          </Tooltip>
        )}
      </Panel>

      <Panel
        icon={<ListChecks />}
        title={t('result.title')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        extra={report && <Badge variant={OVERALL_VARIANT[report.overall]}>{t(`overall.${report.overall}`)}</Badge>}
        footer={
          report && (
            <div className="flex w-full items-center gap-2">
              <Button size="sm" variant="soft" onClick={() => void exportReport('markdown', false)}>
                {t('export.markdown')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('json', false)}>
                {t('export.json')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="sm" onClick={() => void exportReport('markdown', true)}>
                  {t('export.saveMd')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('json', true)}>
                  {t('export.saveJson')}
                </Button>
              </span>
            </div>
          )
        }
      >
        {task.error && <p className="text-xs text-danger" role="alert">{errorMessage(task.error)}</p>}
        {task.status === 'cancelled' && (
          <p className="text-xs text-fg-muted" role="status">{t('errors.bin.cancelled')}</p>
        )}
        {!report && !task.error && task.status !== 'cancelled' && (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<Binary />} title={t('result.empty')} description={t('input.empty')} />
          </div>
        )}
        {report && (
          <>
            {report.incomplete && (
              <p className="rounded-control border border-warning bg-warning/10 p-2.5 text-xs text-warning" role="alert">
                {t('result.incomplete')}
              </p>
            )}
            <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
              <span className="rounded-control bg-active px-2 py-1">{t('result.stats.nodes')}: {report.stats.nodes}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.error')}: {report.stats.errors}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.warning')}: {report.stats.warnings}</span>
              <span className="rounded-control bg-active px-2 py-1">{t('result.level.hint')}: {report.stats.hints}</span>
            </div>
            {report.trailing && (
              <div className="rounded-control border border-warning bg-warning/10 p-2.5 text-xs text-warning">
                <p className="font-medium">
                  {t('result.trailing')}: {t('result.offsetAt', { offset: report.trailing.offset, length: report.trailing.length })}
                </p>
                <p className="mt-1">{t('result.trailingHint')}</p>
              </div>
            )}
            {report.diagnostics.length > 0 && (
              <section className="space-y-1.5">
                <h3 className="text-xs font-semibold text-fg">{t('result.diagnostics')}</h3>
                {report.diagnostics.map((diag: Diag, index) => (
                  <div key={index} className="rounded-control border border-border bg-surface-2 p-2.5">
                    <span className="mb-1 flex flex-wrap items-center gap-2 text-xs">
                      <Badge variant={LEVEL_VARIANT[diag.level]}>{t(`result.level.${diag.level}`)}</Badge>
                      <code className="font-mono text-fg-muted">{diag.code}</code>
                      {diag.offset !== undefined && diag.offset !== null && (
                        <span className="text-fg-subtle">{t('result.offsetAt', { offset: diag.offset, length: 0 })}</span>
                      )}
                    </span>
                    <p className="text-xs text-fg">{errorMessage(diag)}</p>
                  </div>
                ))}
              </section>
            )}
            {report.root ? (
              <section className="space-y-1.5">
                <h3 className="text-xs font-semibold text-fg">{t('result.title')}</h3>
                <TreeNode
                  node={report.root}
                  path="/"
                  selected={selected}
                  onSelect={setSelected}
                  expanded={expanded}
                  toggle={toggle}
                />
              </section>
            ) : (
              <Empty icon={<Binary />} title={t('result.empty')} />
            )}
            {selected && (
              <section className="space-y-1.5">
                <h3 className="text-xs font-semibold text-fg">{t('input.selected')}</h3>
                <p className="text-xs text-fg-muted">
                  {t('result.offsetAt', { offset: selected.offset, length: selected.length })}
                </p>
                {selected.hex && (
                  <pre className="overflow-x-auto rounded-control bg-active px-3 py-2 font-mono text-xs break-all text-fg" data-selectable>
                    {selected.hex}
                    {selected.hexTruncated ? ` … (${t('result.hexTruncated', { count: selected.hex.length / 2 })})` : ''}
                  </pre>
                )}
              </section>
            )}
          </>
        )}
      </Panel>
    </div>
  )
}
