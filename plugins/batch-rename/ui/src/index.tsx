import { useEffect, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, DropdownMenu, Empty, Panel, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowRight, FilePlus2, FolderPlus, ListChecks, Pencil, Plus, Undo2, Upload, Wand2, X } from 'lucide-react'
import { RuleCard } from './RuleCard'
import { KINDS, newRule, type Batch, type Item, type Kind, type Move, type Plan, type Sort, type Step } from './types'

const ROW_HEIGHT = 44
const DEFAULT_RULES: Step[] = [{ ...newRule('replace'), id: 'r1', enabled: true }]
const STATUS_BADGE: Record<Item['status'], 'neutral' | 'success' | 'warning' | 'danger'> = {
  unchanged: 'neutral',
  ok: 'success',
  conflict: 'danger',
  invalid: 'danger',
  missing: 'warning',
}

let nextId = 0
const ruleId = () => `r${Date.now().toString(36)}${nextId++}`

function PreviewRow({ item }: { item: Item }) {
  const { t } = usePlugin()
  const changed = item.status !== 'unchanged' && item.status !== 'missing'
  return (
    <div className="grid h-full grid-cols-[minmax(0,1fr)_16px_minmax(0,1fr)_auto] items-center gap-2 border-b border-border px-3">
      <div className="min-w-0">
        <p className="truncate text-[13px] text-fg" title={item.from}>
          {item.from}
        </p>
        <p className="truncate text-[11px] text-fg-subtle" title={item.path}>
          {item.path}
        </p>
      </div>
      <ArrowRight className={cn('size-3.5', changed ? 'text-fg-muted' : 'text-fg-subtle')} />
      <p className={cn('truncate text-[13px]', item.status === 'ok' ? 'font-medium text-primary' : item.status === 'unchanged' ? 'text-fg-subtle' : 'text-danger')} title={item.to} data-selectable>
        {item.to || '∅'}
      </p>
      {item.status === 'ok' || item.status === 'unchanged' ? (
        <Badge variant={STATUS_BADGE[item.status]}>{t(`status.${item.status}`)}</Badge>
      ) : (
        <Tooltip content={t(`reasons.${item.reason ?? 'unknown'}`)}>
          <Badge variant={STATUS_BADGE[item.status]}>{t(`status.${item.status}`)}</Badge>
        </Tooltip>
      )}
    </div>
  )
}

function BatchRename() {
  const { t, call, errorMessage } = usePlugin()
  const [rules, setRules] = usePluginState<Step[]>('rules', DEFAULT_RULES)
  const [sort, setSort] = usePluginState<Sort>('sort', 'name')
  const [last, setLast] = usePluginState<Batch | null>('lastBatch', null)
  const [paths, setPaths] = useState<string[]>([])
  const [recursive, setRecursive] = useState(false)
  const [hovering, setHovering] = useState(false)
  const applying = useTask<{ journal: Move[] }>()
  const undoing = useTask<{ journal: Move[] }>()
  const listRef = useRef<HTMLDivElement>(null)

  const request = paths.length > 0 ? { paths, rules, sort } : null
  const preview = useDebouncedCall<Plan>(request ? 'preview' : null, request, [paths, rules, sort])
  const plan = paths.length > 0 ? preview.result : null

  const addPaths = async (input: string[]) => {
    if (input.length === 0) return
    try {
      const scanned = await call<{ paths: string[]; skipped: number }>('scan', { paths: input, recursive })
      setPaths((prev) => [...prev, ...scanned.paths.filter((p) => !prev.includes(p))])
      if (scanned.skipped > 0) toast.info(t('files.skipped', { count: scanned.skipped }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const addRef = useRef(addPaths)
  useEffect(() => {
    addRef.current = addPaths
  })

  useEffect(() => {
    const off = host.onFileDrop({ drop: (dropped) => void addRef.current(dropped), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  // 改名或撤销完成后，把列表中的路径换成新路径
  const replacePaths = (journal: Move[]) => {
    const map = new Map(journal.map((m) => [m.from, m.to]))
    setPaths((prev) => prev.map((p) => map.get(p) ?? p))
  }
  // 每个任务单独记录已处理的结果，避免两个任务的结果交替触发重渲染
  const [appliedResult, setAppliedResult] = useState<unknown>(null)
  const [undoneResult, setUndoneResult] = useState<unknown>(null)
  if (applying.status === 'succeeded' && applying.result && applying.result !== appliedResult) {
    setAppliedResult(applying.result)
    replacePaths(applying.result.journal)
    setLast({ journal: applying.result.journal, at: Date.now() })
  }
  if (undoing.status === 'succeeded' && undoing.result && undoing.result !== undoneResult) {
    setUndoneResult(undoing.result)
    replacePaths(undoing.result.journal)
    setLast(null)
  }
  useEffect(() => {
    if (applying.status === 'succeeded') toast.success(t('apply.done', { count: applying.result?.journal.length ?? 0 }))
    if (undoing.status === 'succeeded') toast.success(t('apply.undone', { count: undoing.result?.journal.length ?? 0 }))
    for (const task of [applying, undoing]) if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [applying.status, undoing.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pickFiles = async () => {
    try {
      await addPaths(await host.dialog.openFiles())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickFolder = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) await addPaths([dir])
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const updateRule = (index: number, step: Step) => setRules(rules.map((r, i) => (i === index ? step : r)))
  const moveRule = (index: number, delta: number) => {
    const next = [...rules]
    const [item] = next.splice(index, 1)
    next.splice(index + delta, 0, item)
    setRules(next)
  }
  const addRule = (kind: Kind) => setRules([...rules, { ...newRule(kind), id: ruleId(), enabled: true } as Step])

  const items = plan?.items ?? []
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  })
  const busy = applying.running || undoing.running
  const invalidRules = preview.error

  return (
    <div className="grid h-full min-h-0 grid-cols-[340px_minmax(0,1fr)] gap-3">
      <Panel
        icon={<Wand2 />}
        title={t('rules.title')}
        actions={
          <DropdownMenu
            trigger={
              <Button size="sm">
                <Plus />
                {t('rules.add')}
              </Button>
            }
            items={KINDS.map((kind) => ({ key: kind, label: t(`kinds.${kind}`), onSelect: () => addRule(kind) }))}
          />
        }
        bodyClassName="space-y-2 overflow-auto p-2.5"
      >
        {rules.length === 0 && <Empty icon={<Wand2 />} title={t('rules.empty')} description={t('rules.emptyHint')} />}
        {rules.map((step, index) => (
          <RuleCard
            key={step.id}
            step={step}
            first={index === 0}
            last={index === rules.length - 1}
            onChange={(next) => updateRule(index, next)}
            onMove={(delta) => moveRule(index, delta)}
            onRemove={() => setRules(rules.filter((_, i) => i !== index))}
          />
        ))}
        {invalidRules && <p className="rounded-control bg-danger-soft px-2.5 py-2 text-xs text-danger">{errorMessage(invalidRules)}</p>}
      </Panel>

      <Panel
        icon={<ListChecks />}
        title={t('files.title')}
        extra={paths.length > 0 && <Badge>{paths.length}</Badge>}
        actions={
          <>
            <Select<Sort>
              size="sm"
              value={sort}
              onValueChange={setSort}
              aria-label={t('files.sort')}
              options={(['added', 'name', 'modified', 'size'] as Sort[]).map((value) => ({ value, label: t(`sorts.${value}`) }))}
            />
            <Button size="sm" onClick={pickFiles} disabled={busy}>
              <FilePlus2 />
              {t('files.add')}
            </Button>
            <Button size="sm" onClick={pickFolder} disabled={busy}>
              <FolderPlus />
              {t('files.addFolder')}
            </Button>
            <Tooltip content={t('files.recursiveHint')}>
              <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                <Switch size="sm" checked={recursive} onCheckedChange={setRecursive} aria-label={t('files.recursive')} />
                {t('files.recursive')}
              </label>
            </Tooltip>
            <Button size="icon-sm" variant="ghost" aria-label={t('common:clear')} disabled={busy || paths.length === 0} onClick={() => setPaths([])}>
              <X />
            </Button>
          </>
        }
        footer={
          <>
            <span className="tabular-nums">
              {plan ? t('files.summary', { changes: plan.changes, problems: plan.problems }) : t('files.none')}
            </span>
            <span className="ml-auto flex items-center gap-2">
              {last && (
                <Tooltip content={t('apply.undoHint', { count: last.journal.length, time: new Date(last.at).toLocaleString() })}>
                  <Button size="sm" onClick={() => undoing.start('undo', { journal: last.journal })} disabled={busy}>
                    <Undo2 />
                    {t('apply.undo')}
                  </Button>
                </Tooltip>
              )}
              <Button
                size="sm"
                variant="primary"
                loading={applying.running}
                disabled={busy || !plan || plan.changes === 0 || plan.problems > 0 || preview.pending}
                onClick={() => applying.start('apply', { paths, rules, sort })}
              >
                <Pencil />
                {t('apply.run', { count: plan?.changes ?? 0 })}
              </Button>
            </span>
          </>
        }
        bodyClassName="p-0"
      >
        {paths.length === 0 ? (
          <div className="h-full p-3">
            <button
              type="button"
              onClick={pickFiles}
              className={cn(
                'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <Upload className="size-6" />
              <span className="text-[13px] font-medium">{t('files.drop')}</span>
              <span className="text-xs text-fg-subtle">{t('files.dropHint')}</span>
            </button>
          </div>
        ) : (
          <div ref={listRef} className={cn('h-full overflow-auto', hovering && 'ring-2 ring-primary ring-inset')}>
            <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
              {virtualizer.getVirtualItems().map((row) => (
                <div key={row.key} className="absolute left-0 w-full hover:bg-hover" style={{ top: row.start, height: ROW_HEIGHT }}>
                  <PreviewRow item={items[row.index]} />
                </div>
              ))}
            </div>
          </div>
        )}
      </Panel>
    </div>
  )
}

export default BatchRename
