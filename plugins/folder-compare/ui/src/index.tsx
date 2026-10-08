import { useEffect, useMemo, useRef, useState, type MouseEvent } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, Empty, Input, Modal, Panel, SegmentedControl, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowLeftRight, ArrowLeftToLine, ArrowRightToLine, ChevronDown, ChevronRight, File, Folder, FolderOpen, GitCompareArrows } from 'lucide-react'
import { DiffView } from './DiffView'
import { STATUS_BADGE, STATUS_TEXT, type Compared, type Filter, type Info, type Item, type Method } from './types'

const ROW_HEIGHT = 30
const FILTERS: Filter[] = ['diff', 'all', 'changed', 'leftOnly', 'rightOnly']
const DEFAULT_IGNORE = '.git, node_modules, .DS_Store, Thumbs.db, desktop.ini'

type Direction = 'toRight' | 'toLeft'

function matches(item: Item, filter: Filter) {
  switch (filter) {
    case 'all':
      return true
    case 'diff':
      return item.status !== 'same'
    case 'changed':
      return item.status === 'changed' || item.status === 'mismatch'
    default:
      return item.status === filter
  }
}

function SideCell({ info, newer }: { info: Info | null; newer: boolean }) {
  // 文件夹的时间没有参考意义，只显示文件的大小与时间
  if (!info || info.dir) return <span />
  return (
    <span className={cn('flex justify-end gap-3 text-[11px] whitespace-nowrap tabular-nums', newer ? 'font-medium text-fg' : 'text-fg-muted')}>
      <span>{formatBytes(info.size)}</span>
      <span>{new Date(info.modified).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' })}</span>
    </span>
  )
}

/** 路径过长时从开头截断，保留末尾的文件夹名 */
function FolderPath({ path, placeholder }: { path: string | null; placeholder: string }) {
  if (!path) return <span className="truncate">{placeholder}</span>
  return (
    <span className="min-w-0 flex-1 truncate text-left [direction:rtl]" title={path}>
      <bdi>{path}</bdi>
    </span>
  )
}

function FolderCompare() {
  const { t, errorMessage } = usePlugin()
  const [left, setLeft] = usePluginState<string | null>('left', null)
  const [right, setRight] = usePluginState<string | null>('right', null)
  const [method, setMethod] = usePluginState<Method>('method', 'quick')
  const [ignoreText, setIgnoreText] = usePluginState('ignore', DEFAULT_IGNORE)
  const [ignoreHidden, setIgnoreHidden] = usePluginState('ignoreHidden', false)
  const [filter, setFilter] = useState<Filter>('diff')
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set())
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [focus, setFocus] = useState<string | null>(null)
  const [confirm, setConfirm] = useState<Direction | null>(null)
  const comparing = useTask<Compared>()
  const copying = useTask<{ files: number; bytes: number }>()
  const listRef = useRef<HTMLDivElement>(null)

  const ignore = ignoreText.split(',').map((s) => s.trim()).filter(Boolean)
  const result = comparing.result
  const items = useMemo(() => result?.items ?? [], [result])

  const compare = () => {
    if (!left || !right) return
    setSelected(new Set())
    setFocus(null)
    comparing.start('compare', { left, right, method, ignore, ignoreHidden })
  }
  const compareRef = useRef(compare)
  useEffect(() => {
    compareRef.current = compare
  })
  useEffect(() => {
    if (comparing.status === 'failed' && comparing.error) toast.error(errorMessage(comparing.error))
  }, [comparing.status]) // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (copying.status === 'succeeded' && copying.result) {
      toast.success(t('copy.done', { count: copying.result.files, size: formatBytes(copying.result.bytes) }))
      compareRef.current()
    }
    if (copying.status === 'failed' && copying.error) toast.error(errorMessage(copying.error))
  }, [copying.status]) // eslint-disable-line react-hooks/exhaustive-deps

  // 文件夹显示条件：自身符合筛选或有符合筛选的子项；折叠的文件夹隐藏其内容
  const visible = useMemo(() => {
    const keep = new Set<string>()
    for (const item of items) {
      if (!matches(item, filter)) continue
      keep.add(item.path)
      let path = item.path
      while (path.includes('/')) {
        path = path.slice(0, path.lastIndexOf('/'))
        keep.add(path)
      }
    }
    return items.filter((item) => {
      if (!keep.has(item.path)) return false
      for (const dir of collapsed) if (item.path.startsWith(`${dir}/`)) return false
      return true
    })
  }, [items, filter, collapsed])

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: visible.length, getScrollElement: () => listRef.current, estimateSize: () => ROW_HEIGHT, overscan: 20 })

  const pick = async (side: 'left' | 'right') => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) (side === 'left' ? setLeft : setRight)(dir)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const swap = () => {
    setLeft(right)
    setRight(left)
  }
  const toggle = (path: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  const onRow = (item: Item, e: MouseEvent) => {
    if (e.metaKey || e.ctrlKey) {
      setSelected((prev) => {
        const next = new Set(prev)
        if (next.has(item.path)) next.delete(item.path)
        else next.add(item.path)
        return next
      })
    } else {
      setSelected(new Set([item.path]))
    }
    setFocus(item.path)
  }

  const chosen = items.filter((i) => selected.has(i.path) && i.status !== 'same' && i.status !== 'mismatch')
  const canRight = chosen.length > 0 && chosen.every((i) => i.left)
  const canLeft = chosen.length > 0 && chosen.every((i) => i.right)
  const overwrites = confirm ? chosen.filter((i) => (confirm === 'toRight' ? i.right : i.left)).length : 0
  const runCopy = () => {
    if (!confirm || !result) return
    copying.start('copy', { left: result.left, right: result.right, paths: chosen.map((i) => i.path), direction: confirm, ignore, ignoreHidden })
    setConfirm(null)
  }
  const focusItem = items.find((i) => i.path === focus)
  const showDiff = result && focusItem && !focusItem.dir && focusItem.status !== 'same'

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel className="shrink-0" bodyClassName="space-y-2.5 p-3">
        <div className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-2">
          <Button className="justify-start" onClick={() => pick('left')}>
            <FolderOpen />
            <FolderPath path={left} placeholder={t('folders.left')} />
          </Button>
          <Tooltip content={t('folders.swap')}>
            <Button size="icon-md" variant="ghost" aria-label={t('folders.swap')} onClick={swap} disabled={!left && !right}>
              <ArrowLeftRight />
            </Button>
          </Tooltip>
          <Button className="justify-start" onClick={() => pick('right')}>
            <FolderOpen />
            <FolderPath path={right} placeholder={t('folders.right')} />
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <SegmentedControl<Method>
            size="sm"
            value={method}
            onValueChange={setMethod}
            aria-label={t('options.method')}
            options={(['quick', 'content'] as Method[]).map((value) => ({ value, label: t(`methods.${value}`) }))}
          />
          <Tooltip content={t('options.ignoreHint')}>
            <Input size="sm" value={ignoreText} onChange={(e) => setIgnoreText(e.target.value)} className="w-80 font-mono" aria-label={t('options.ignore')} placeholder={t('options.ignore')} />
          </Tooltip>
          <label className="flex items-center gap-1.5 text-xs text-fg-muted">
            <Switch size="sm" checked={ignoreHidden} onCheckedChange={setIgnoreHidden} aria-label={t('options.hidden')} />
            {t('options.hidden')}
          </label>
          <Button variant="primary" size="sm" className="ml-auto" loading={comparing.running} disabled={!left || !right || copying.running} onClick={compare}>
            <GitCompareArrows />
            {t('compare.run')}
          </Button>
        </div>
      </Panel>

      <Panel
        icon={<GitCompareArrows />}
        title={t('result.title')}
        extra={
          result && (
            <>
              {(['changed', 'leftOnly', 'rightOnly', 'mismatch', 'same'] as const).map((key) =>
                result.summary[key] > 0 ? (
                  <Badge key={key} variant={STATUS_BADGE[key]}>
                    {t(`status.${key}`)} {result.summary[key]}
                  </Badge>
                ) : null,
              )}
            </>
          )
        }
        actions={
          result && (
            <>
              <SegmentedControl<Filter>
                size="sm"
                value={filter}
                onValueChange={setFilter}
                aria-label={t('result.filter')}
                options={FILTERS.map((value) => ({ value, label: t(`filters.${value}`) }))}
              />
              <Button size="sm" disabled={!canRight || copying.running} loading={copying.running} onClick={() => setConfirm('toRight')}>
                <ArrowRightToLine />
                {t('copy.toRight')}
              </Button>
              <Button size="sm" disabled={!canLeft || copying.running} onClick={() => setConfirm('toLeft')}>
                <ArrowLeftToLine />
                {t('copy.toLeft')}
              </Button>
            </>
          )
        }
        className="min-h-0 flex-[3]"
        bodyClassName="p-0"
      >
        {!result ? (
          <Empty icon={<GitCompareArrows />} title={t('result.empty')} description={t('result.emptyHint')} />
        ) : visible.length === 0 ? (
          <Empty icon={<GitCompareArrows />} title={result.summary.changed + result.summary.leftOnly + result.summary.rightOnly + result.summary.mismatch === 0 ? t('result.identical') : t('result.noMatch')} />
        ) : (
          <div className="flex h-full min-h-0 flex-col">
            <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_200px_96px_200px] gap-3 border-b border-border px-3 py-1.5 text-[11px] text-fg-subtle">
              <span>{t('result.name')}</span>
              <span className="truncate text-right" title={result.left}>{t('folders.left')}</span>
              <span className="text-center">{t('result.status')}</span>
              <span className="truncate text-right" title={result.right}>{t('folders.right')}</span>
            </div>
            <div ref={listRef} className="min-h-0 flex-1 overflow-auto">
              <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {virtualizer.getVirtualItems().map((row) => {
                  const item = visible[row.index]
                  const open = !collapsed.has(item.path)
                  return (
                    <div
                      key={item.path}
                      role="row"
                      onMouseDown={(e) => onRow(item, e)}
                      className={cn(
                        'absolute left-0 grid w-full cursor-default grid-cols-[minmax(0,1fr)_200px_96px_200px] items-center gap-3 px-3',
                        selected.has(item.path) ? 'bg-active' : 'hover:bg-hover',
                      )}
                      style={{ top: row.start, height: ROW_HEIGHT }}
                    >
                      <span className="flex min-w-0 items-center gap-1.5" style={{ paddingLeft: item.depth * 16 }}>
                        {item.dir ? (
                          <button
                            type="button"
                            aria-label={t(open ? 'result.collapse' : 'result.expand')}
                            className="grid size-4 place-items-center text-fg-subtle"
                            onMouseDown={(e) => e.stopPropagation()}
                            onClick={() => toggle(item.path)}
                          >
                            {open ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
                          </button>
                        ) : (
                          <span className="size-4" />
                        )}
                        {item.dir ? <Folder className={cn('size-4 shrink-0', STATUS_TEXT[item.status])} /> : <File className={cn('size-4 shrink-0', STATUS_TEXT[item.status])} />}
                        <span className={cn('truncate text-[13px]', item.status === 'same' ? 'text-fg-muted' : 'text-fg')} title={item.path}>
                          {item.name}
                        </span>
                      </span>
                      <SideCell info={item.left} newer={item.newer === 'left'} />
                      <span className="text-center">
                        <Badge variant={STATUS_BADGE[item.status]}>{t(`status.${item.status}`)}</Badge>
                      </span>
                      <SideCell info={item.right} newer={item.newer === 'right'} />
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        )}
      </Panel>

      {showDiff && <DiffView left={result.left} right={result.right} path={focusItem.path} />}

      <Modal
        open={confirm !== null}
        onOpenChange={(open) => !open && setConfirm(null)}
        title={t(confirm === 'toLeft' ? 'copy.confirmLeft' : 'copy.confirmRight', { count: chosen.length })}
        description={overwrites > 0 ? t('copy.overwrite', { count: overwrites }) : t('copy.noOverwrite')}
        footer={
          <>
            <Button onClick={() => setConfirm(null)}>{t('common:cancel')}</Button>
            <Button variant={overwrites > 0 ? 'danger' : 'primary'} onClick={runCopy}>
              {t('copy.confirm')}
            </Button>
          </>
        }
      >
        <ul className="max-h-48 space-y-0.5 overflow-auto px-5 pb-4 font-mono text-xs text-fg-muted">
          {chosen.slice(0, 50).map((item) => (
            <li key={item.path} className="truncate">
              {item.path}
              {item.dir ? '/' : ''}
            </li>
          ))}
          {chosen.length > 50 && <li>{t('copy.more', { count: chosen.length - 50 })}</li>}
        </ul>
      </Modal>
    </div>
  )
}

export default FolderCompare
