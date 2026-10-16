import { useEffect, useMemo, useRef, useState } from 'react'
import { Badge, Button, Empty, Modal, Panel, SegmentedControl, Select, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Columns3, FilePlus2, GitMerge, Star, X } from 'lucide-react'
import { ENV_EXTENSIONS, baseName, type Cell, type Comparison, type Issue, type MergeResult } from './types'

/** 一次最多对比的文件数，与后端 MAX_COMPARE 一致 */
const MAX_FILES = 8
const ISSUES: Issue[] = ['missing', 'extra', 'empty', 'duplicate', 'differs']
const ISSUE_VARIANT: Record<Issue, 'danger' | 'warning' | 'info' | 'neutral'> = {
  missing: 'danger',
  extra: 'info',
  empty: 'warning',
  duplicate: 'warning',
  differs: 'neutral',
}

type Filter = 'issues' | 'all'

export function Compare({ active }: { active: boolean }) {
  const { t, call, errorMessage } = usePlugin()
  const [paths, setPaths] = useState<string[]>([])
  const [result, setResult] = useState<Comparison | null>(null)
  const [filter, setFilter] = useState<Filter>('issues')
  const [loading, setLoading] = useState(false)
  const [target, setTarget] = useState<string | null>(null)
  const [fill, setFill] = useState(false)
  const [preview, setPreview] = useState<MergeResult | null>(null)
  const [merging, setMerging] = useState(false)
  const [dropping, setDropping] = useState(false)

  const run = async (next: string[]) => {
    setPaths(next)
    setResult(null)
    if (target && !next.slice(1).includes(target)) setTarget(null)
    if (next.length < 2) return
    setLoading(true)
    try {
      setResult(await call<Comparison>('compare', { paths: next }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setLoading(false)
    }
  }

  const add = (added: string[]) => {
    const merged = [...paths, ...added.filter((path) => !paths.includes(path))]
    if (merged.length > MAX_FILES) toast.info(t('compare.tooMany', { max: MAX_FILES }))
    void run(merged.slice(0, MAX_FILES))
  }

  const pick = async () => {
    try {
      const files = await host.dialog.openFiles([{ name: t('dialog.filter'), extensions: ENV_EXTENSIONS }])
      if (files.length > 0) add(files)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  // 只有当前页可见时才接收拖入的文件
  const addRef = useRef(add)
  useEffect(() => {
    addRef.current = add
  })
  useEffect(() => {
    if (!active) return
    const off = host.onFileDrop({ drop: (dropped) => addRef.current(dropped), over: setDropping })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [active])

  const rows = useMemo(() => (result ? (filter === 'all' ? result.rows : result.rows.filter((row) => row.issues.length > 0)) : []), [result, filter])

  const openPreview = async () => {
    if (!target || paths.length === 0) return
    setMerging(true)
    try {
      setPreview(await call<MergeResult>('merge_preview', { base: paths[0], target, fill }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setMerging(false)
    }
  }

  const applyMerge = async () => {
    if (!target) return
    setMerging(true)
    try {
      const merged = await call<MergeResult>('merge', { base: paths[0], target, fill })
      setPreview(null)
      toast.success(t('compare.merged', { count: merged.added.length, file: baseName(target) }))
      await run(paths)
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setMerging(false)
    }
  }

  return (
    <Panel
      icon={<Columns3 />}
      title={t('compare.title')}
      actions={
        <Button size="sm" onClick={pick} disabled={loading || paths.length >= MAX_FILES}>
          <FilePlus2 />
          {t('compare.add')}
        </Button>
      }
      footer={
        result && paths.length >= 2 ? (
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-fg-muted">{t('compare.mergeInto')}</span>
            <Select<string>
              size="sm"
              className="w-56"
              value={target}
              onValueChange={setTarget}
              placeholder={t('compare.pickTarget')}
              aria-label={t('compare.mergeInto')}
              options={paths.slice(1).map((path) => ({ value: path, label: baseName(path) }))}
            />
            <label className="flex items-center gap-1.5 text-xs text-fg-muted">
              <Switch checked={fill} onCheckedChange={setFill} aria-label={t('compare.fill')} />
              {t('compare.fill')}
            </label>
            <Button size="sm" variant="primary" className="ml-auto" disabled={!target || merging} loading={merging && preview === null} onClick={openPreview}>
              <GitMerge />
              {t('compare.preview')}
            </Button>
          </div>
        ) : undefined
      }
      className="h-full"
      bodyClassName={cn('flex min-h-0 flex-col gap-3 overflow-auto p-3', dropping && 'ring-2 ring-primary')}
    >
      {paths.length === 0 ? (
        <button
          type="button"
          onClick={pick}
          className={cn(
            'flex min-h-64 flex-1 flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
            'focus-visible:ring-2 focus-visible:ring-ring',
            dropping ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
          )}
        >
          <Columns3 className="size-6" />
          <span className="text-[13px] font-medium">{t('compare.drop')}</span>
          <span className="max-w-md text-xs text-fg-subtle">{t('compare.rules')}</span>
        </button>
      ) : (
        <>
          <ul className="flex flex-wrap gap-2">
            {paths.map((path, index) => {
              const summary = result?.summaries[index]
              return (
                <li key={path} className={cn('flex min-w-0 items-center gap-2 rounded-control border px-2.5 py-1.5', index === 0 ? 'border-primary bg-primary-soft' : 'border-border bg-surface')}>
                  <div className="min-w-0">
                    <p className="flex items-center gap-1 truncate text-[13px] font-medium text-fg" title={path}>
                      {index === 0 && <Star className="size-3.5 shrink-0 text-primary" aria-hidden />}
                      {baseName(path)}
                    </p>
                    {summary && (
                      <p className="text-[11px] text-fg-subtle tabular-nums">
                        {t('compare.summary', { entries: summary.entries, missing: summary.missing, extra: summary.extra, empty: summary.empty })}
                      </p>
                    )}
                  </div>
                  {index > 0 && (
                    <Tooltip content={t('compare.setBase')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('compare.setBase')} onClick={() => void run([path, ...paths.filter((p) => p !== path)])}>
                        <Star />
                      </Button>
                    </Tooltip>
                  )}
                  <Tooltip content={t('compare.remove')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('compare.remove')} onClick={() => void run(paths.filter((p) => p !== path))}>
                      <X />
                    </Button>
                  </Tooltip>
                </li>
              )
            })}
          </ul>

          {paths.length < 2 ? (
            <Empty className="py-10" icon={<Columns3 />} title={t('compare.needTwo')} description={t('compare.rules')} />
          ) : result ? (
            <>
              <div className="flex flex-wrap items-center gap-2">
                <SegmentedControl<Filter>
                  size="sm"
                  value={filter}
                  onValueChange={setFilter}
                  aria-label={t('compare.filter')}
                  options={[
                    { value: 'issues', label: t('compare.onlyIssues') },
                    { value: 'all', label: t('compare.all', { count: result.rows.length }) },
                  ]}
                />
                <div className="ml-auto flex flex-wrap gap-1.5">
                  {ISSUES.map((issue) => (
                    <Tooltip key={issue} content={t(`compare.issueHint.${issue}`)}>
                      <Badge variant={ISSUE_VARIANT[issue]}>{t(`compare.issue.${issue}`)}</Badge>
                    </Tooltip>
                  ))}
                </div>
              </div>
              {rows.length === 0 ? (
                <Empty className="py-10" icon={<Columns3 />} title={t('compare.consistent')} />
              ) : (
                <div className="overflow-auto rounded-control border border-border">
                  <table className="w-full border-collapse text-[13px]">
                    <thead className="sticky top-0 bg-surface-2 text-xs text-fg-muted">
                      <tr>
                        <th className="px-3 py-2 text-left font-medium">{t('row.key')}</th>
                        {paths.map((path) => (
                          <th key={path} className="px-3 py-2 text-left font-medium" title={path}>
                            {baseName(path)}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((row) => (
                        <tr key={row.key} className="border-t border-border">
                          <td className="px-3 py-1.5 font-mono" data-selectable>
                            {row.key}
                          </td>
                          {row.cells.map((cell, index) => (
                            <td key={paths[index]} className="px-3 py-1.5">
                              <CellBadges cell={cell} isBase={index === 0} baseHasKey={row.cells[0].state !== 'missing'} />
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </>
          ) : null}
        </>
      )}

      <Modal
        open={preview !== null}
        onOpenChange={(open) => !open && setPreview(null)}
        size="lg"
        title={t('compare.previewTitle', { file: target ? baseName(target) : '' })}
        description={preview && preview.added.length > 0 ? t('compare.previewAdded', { keys: preview.added.join(', ') }) : t('compare.nothingToMerge')}
        closeLabel={t('compare.cancel')}
        footer={
          <>
            <Button onClick={() => setPreview(null)}>{t('compare.cancel')}</Button>
            <Button variant="primary" disabled={!preview || preview.added.length === 0} loading={merging} onClick={applyMerge}>
              <GitMerge />
              {t('compare.apply')}
            </Button>
          </>
        }
      >
        {preview && (
          <pre className="max-h-[60vh] overflow-auto rounded-control border border-border bg-surface-2 p-3 font-mono text-xs text-fg" data-selectable>
            {preview.text}
          </pre>
        )}
      </Modal>
    </Panel>
  )
}

function CellBadges({ cell, isBase, baseHasKey }: { cell: Cell; isBase: boolean; baseHasKey: boolean }) {
  const { t } = usePlugin()
  if (cell.state === 'missing') {
    // 基准缺少的键不是“缺失”，而是其它文件的“额外键”
    return isBase ? <span className="text-xs text-fg-subtle">{t('compare.absent')}</span> : <Badge variant="danger">{t('compare.issue.missing')}</Badge>
  }
  return (
    <div className="flex flex-wrap gap-1">
      {cell.state === 'empty' ? <Badge variant="warning">{t('compare.issue.empty')}</Badge> : <span className="text-xs text-fg-muted">{t('compare.set')}</span>}
      {!isBase && !baseHasKey && <Badge variant="info">{t('compare.issue.extra')}</Badge>}
      {cell.duplicate && <Badge variant="warning">{t('compare.issue.duplicate')}</Badge>}
      {cell.sameAsBase === false && <Badge>{t('compare.issue.differs')}</Badge>}
    </div>
  )
}
