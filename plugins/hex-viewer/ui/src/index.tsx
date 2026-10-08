import { useEffect, useRef, useState, type KeyboardEvent } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, Checkbox, DropdownMenu, Empty, Input, Panel, SegmentedControl, cn, toast } from '@toolforge/ui'
import { formatBytes, host, useCopy, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, Binary, ClipboardCopy, CornerDownRight, FileSearch, FolderOpen, ScanSearch } from 'lucide-react'

const ROW = 16
const ROW_HEIGHT = 22
const PAGE_ROWS = 256
const PAGE_BYTES = ROW * PAGE_ROWS
const COLUMNS = Array.from({ length: ROW }, (_, i) => i)
const FORMATS = ['hex', 'hexCompact', 'base64', 'c', 'python', 'text'] as const

type Mode = 'hex' | 'text'
interface Info {
  path: string
  name: string
  size: number
  kind: string | null
  offsetWidth: number
}
interface Row {
  offset: number
  label: string
  hex: string[]
  ascii: string[]
}
interface Value {
  kind: string
  value: string
}
interface Match {
  offset: number
  length: number
}

/** 状态栏中的偏移量 */
const hexOffset = (n: number) => `0x${n.toString(16).toUpperCase()}`

function HexViewer() {
  const { t, call, errorMessage } = usePlugin()
  const { copy } = useCopy()
  const [littleEndian, setLittleEndian] = usePluginState('littleEndian', true)
  const [mode, setMode] = usePluginState<Mode>('searchMode', 'text')
  const [ignoreCase, setIgnoreCase] = usePluginState('ignoreCase', true)
  const [info, setInfo] = useState<Info | null>(null)
  const [pages, setPages] = useState<Record<number, Row[]>>({})
  const [cursor, setCursor] = useState(0)
  const [anchor, setAnchor] = useState<number | null>(null)
  const [match, setMatch] = useState<Match | null>(null)
  const [query, setQuery] = useState('')
  const [target, setTarget] = useState('')
  const [searching, setSearching] = useState(false)
  const [hovering, setHovering] = useState(false)
  const listRef = useRef<HTMLDivElement>(null)
  const requested = useRef(new Set<number>())

  const open = async (path: string) => {
    try {
      const next = await call<Info>('open', { path })
      requested.current = new Set()
      setPages({})
      setInfo(next)
      setCursor(0)
      setAnchor(null)
      setMatch(null)
      listRef.current?.scrollTo({ top: 0 })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const openRef = useRef(open)
  useEffect(() => {
    openRef.current = open
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => paths[0] && void openRef.current(paths[0]), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  const size = info?.size ?? 0
  const rowCount = Math.ceil(size / ROW)
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => listRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  })
  const items = virtualizer.getVirtualItems()
  const firstPage = items.length ? Math.floor(items[0].index / PAGE_ROWS) : 0
  const lastPage = items.length ? Math.floor(items[items.length - 1].index / PAGE_ROWS) : -1

  // 按页读取可见区域的数据
  useEffect(() => {
    if (!info) return
    for (let page = firstPage; page <= lastPage; page++) {
      if (requested.current.has(page)) continue
      requested.current.add(page)
      call<Row[]>('read', { path: info.path, offset: page * PAGE_BYTES, length: PAGE_BYTES })
        .then((rows) => setPages((prev) => ({ ...prev, [page]: rows })))
        .catch((error) => {
          requested.current.delete(page)
          toast.error(errorMessage(error))
        })
    }
  }, [info, firstPage, lastPage, call, errorMessage])

  const start = anchor === null ? cursor : Math.min(anchor, cursor)
  const end = anchor === null ? cursor : Math.max(anchor, cursor)
  // 有选区时从选区开头解读
  const inspectArgs = info && size > 0 ? { path: info.path, offset: start, littleEndian } : null
  const inspector = useDebouncedCall<Value[]>(inspectArgs ? 'inspect' : null, inspectArgs, [info?.path, start, littleEndian])
  const selectionLength = size > 0 ? end - start + 1 : 0

  const moveTo = (offset: number, extend = false, align: 'auto' | 'center' = 'auto') => {
    const next = Math.max(0, Math.min(size - 1, offset))
    if (extend) setAnchor((prev) => prev ?? cursor)
    else setAnchor(null)
    setCursor(next)
    virtualizer.scrollToIndex(Math.floor(next / ROW), { align })
  }
  const select = (from: number, length: number) => {
    setAnchor(from)
    setCursor(from + length - 1)
    virtualizer.scrollToIndex(Math.floor(from / ROW), { align: 'center' })
  }

  const onKeyDown = (e: KeyboardEvent) => {
    const page = Math.max(1, Math.floor((listRef.current?.clientHeight ?? 400) / ROW_HEIGHT) - 1) * ROW
    const delta: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -ROW, ArrowDown: ROW, PageUp: -page, PageDown: page }
    if (e.key in delta) moveTo(cursor + delta[e.key], e.shiftKey)
    else if (e.key === 'Home') moveTo(e.metaKey || e.ctrlKey ? 0 : cursor - (cursor % ROW), e.shiftKey)
    else if (e.key === 'End') moveTo(e.metaKey || e.ctrlKey ? size - 1 : cursor - (cursor % ROW) + ROW - 1, e.shiftKey)
    else return
    e.preventDefault()
  }

  const pick = async () => {
    try {
      const path = await host.dialog.openFile()
      if (path) await open(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const find = async (backward: boolean) => {
    if (!info || !query) return
    setSearching(true)
    try {
      const args = { path: info.path, query, mode, ignoreCase }
      const from = backward ? start : match && match.offset === start ? start + 1 : cursor
      let found = await call<{ offset: number | null; length: number }>('find', { ...args, from, backward })
      // 到头后从另一端继续
      if (found.offset === null) {
        found = await call('find', { ...args, from: backward ? size : 0, backward })
        if (found.offset !== null) toast.info(t(backward ? 'search.wrappedEnd' : 'search.wrappedStart'))
      }
      if (found.offset === null) {
        setMatch(null)
        toast.info(t('search.notFound'))
      } else {
        setMatch({ offset: found.offset, length: found.length })
        select(found.offset, found.length)
      }
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setSearching(false)
    }
  }
  const goto = async () => {
    if (!info || !target.trim()) return
    try {
      const { offset } = await call<{ offset: number }>('goto', { expr: target, current: cursor, size })
      moveTo(offset, false, 'center')
      listRef.current?.focus()
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const copyAs = async (format: (typeof FORMATS)[number]) => {
    if (!info) return
    try {
      const { text } = await call<{ text: string }>('copy', { path: info.path, offset: start, length: selectionLength, format })
      await copy(text)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const cellClass = (offset: number) =>
    cn(
      offset === cursor && 'bg-primary text-fg-on-primary',
      offset !== cursor && offset >= start && offset <= end && anchor !== null && 'bg-primary-soft text-primary-fg',
      offset !== cursor && anchor === null && match && offset >= match.offset && offset < match.offset + match.length && 'bg-warning-soft',
    )
  const onCell = (offset: number, shift: boolean) => {
    if (shift) setAnchor((prev) => prev ?? cursor)
    else setAnchor(null)
    setCursor(offset)
    listRef.current?.focus()
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_300px] gap-3">
      <Panel
        icon={<Binary />}
        title={info ? info.name : t('file.title')}
        extra={info && (
          <>
            {info.kind && <Badge variant="info">{info.kind}</Badge>}
            <Badge>{formatBytes(info.size)}</Badge>
          </>
        )}
        actions={
          <Button size="sm" onClick={pick}>
            <FolderOpen />
            {t('file.open')}
          </Button>
        }
        footer={
          info && size > 0 && (
            <span className="font-mono tabular-nums">
              {t('status.cursor', { offset: hexOffset(cursor), decimal: cursor })}
              {anchor !== null && ` · ${t('status.selection', { count: selectionLength, from: hexOffset(start), to: hexOffset(end) })}`}
            </span>
          )
        }
        bodyClassName="p-0"
      >
        {!info ? (
          <div className="h-full p-3">
            <button
              type="button"
              onClick={pick}
              className={cn(
                'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <FileSearch className="size-6" />
              <span className="text-[13px] font-medium">{t('file.drop')}</span>
              <span className="text-xs text-fg-subtle">{t('file.dropHint')}</span>
            </button>
          </div>
        ) : size === 0 ? (
          <Empty icon={<Binary />} title={t('file.empty')} />
        ) : (
          <div className="flex h-full min-h-0 flex-col font-mono text-[12px]">
            <div className="flex shrink-0 items-center gap-4 border-b border-border px-3 py-1.5 text-fg-subtle">
              <span style={{ width: `${info.offsetWidth}ch` }}>{t('file.offset')}</span>
              <span className="flex">
                {COLUMNS.map((i) => (
                  <span key={i} className={cn('w-[2.6ch] text-center', i === 8 && 'ml-[1ch]')}>
                    {i.toString(16).toUpperCase().padStart(2, '0')}
                  </span>
                ))}
              </span>
              <span>{t('file.ascii')}</span>
            </div>
            <div
              ref={listRef}
              tabIndex={0}
              onKeyDown={onKeyDown}
              className={cn('min-h-0 flex-1 overflow-auto outline-none', hovering && 'ring-2 ring-primary ring-inset')}
            >
              <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                {items.map((item) => {
                  const page = pages[Math.floor(item.index / PAGE_ROWS)]
                  const row = page?.[item.index % PAGE_ROWS]
                  const base = item.index * ROW
                  return (
                    <div
                      key={item.key}
                      className="absolute left-0 flex w-full items-center gap-4 px-3 hover:bg-hover"
                      style={{ top: item.start, height: ROW_HEIGHT }}
                    >
                      <span className={cn('text-fg-subtle', Math.floor(cursor / ROW) === item.index && 'text-primary')} style={{ width: `${info.offsetWidth}ch` }}>
                        {row?.label}
                      </span>
                      <span className="flex">
                        {COLUMNS.map((i) => (
                          <span
                            key={i}
                            onMouseDown={(e) => row?.hex[i] && onCell(base + i, e.shiftKey)}
                            className={cn('w-[2.6ch] cursor-default rounded-[3px] text-center text-fg', i === 8 && 'ml-[1ch]', row?.hex[i] && cellClass(base + i))}
                          >
                            {row?.hex[i] ?? ''}
                          </span>
                        ))}
                      </span>
                      <span className="flex">
                        {COLUMNS.map((i) => (
                          <span
                            key={i}
                            onMouseDown={(e) => row?.ascii[i] && onCell(base + i, e.shiftKey)}
                            className={cn('w-[1.2ch] cursor-default text-center text-fg-muted', row?.ascii[i] && cellClass(base + i))}
                          >
                            {row?.ascii[i] ?? ''}
                          </span>
                        ))}
                      </span>
                    </div>
                  )
                })}
              </div>
            </div>
          </div>
        )}
      </Panel>

      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <Panel icon={<ScanSearch />} title={t('search.title')} className="shrink-0" bodyClassName="space-y-2.5 p-3">
          <div className="flex gap-2">
            <Input
              size="sm"
              value={target}
              onChange={(e) => setTarget(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void goto()}
              placeholder={t('goto.placeholder')}
              className="font-mono"
              aria-label={t('goto.title')}
              disabled={!info}
            />
            <Button size="sm" onClick={goto} disabled={!info || !target.trim()}>
              <CornerDownRight />
              {t('goto.go')}
            </Button>
          </div>
          <div className="space-y-2 border-t border-border pt-2.5">
            <SegmentedControl<Mode>
              size="sm"
              value={mode}
              onValueChange={setMode}
              aria-label={t('search.mode')}
              options={(['text', 'hex'] as Mode[]).map((value) => ({ value, label: t(`search.modes.${value}`) }))}
            />
            <Input
              size="sm"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void find(e.shiftKey)}
              placeholder={t(`search.placeholder.${mode}`)}
              className={cn(mode === 'hex' && 'font-mono')}
              aria-label={t('search.title')}
              disabled={!info}
            />
            <div className="flex items-center gap-2">
              {mode === 'text' && (
                <Checkbox checked={ignoreCase} onCheckedChange={setIgnoreCase}>
                  <span className="text-xs text-fg-muted">{t('search.ignoreCase')}</span>
                </Checkbox>
              )}
              <span className="ml-auto flex gap-1">
                <Button size="sm" onClick={() => find(true)} disabled={!info || !query || searching} aria-label={t('search.previous')}>
                  <ArrowUp />
                </Button>
                <Button size="sm" variant="primary" loading={searching} onClick={() => find(false)} disabled={!info || !query}>
                  <ArrowDown />
                  {t('search.next')}
                </Button>
              </span>
            </div>
          </div>
        </Panel>

        <Panel
          icon={<Binary />}
          title={t('inspector.title')}
          actions={
            <SegmentedControl<'le' | 'be'>
              size="sm"
              value={littleEndian ? 'le' : 'be'}
              onValueChange={(v) => setLittleEndian(v === 'le')}
              aria-label={t('inspector.endian')}
              options={[
                { value: 'le', label: t('inspector.le') },
                { value: 'be', label: t('inspector.be') },
              ]}
            />
          }
          className="shrink-0"
          bodyClassName="p-0"
        >
          {!inspector.result || inspector.result.length === 0 ? (
            <p className="p-3 text-xs text-fg-subtle">{t('inspector.empty')}</p>
          ) : (
            <table className="w-full text-xs">
              <tbody>
                {inspector.result.map((v) => (
                  <tr key={v.kind} className="border-b border-border last:border-0">
                    <th className="w-24 px-3 py-1.5 text-left font-normal text-fg-muted">{t(`inspector.kinds.${v.kind}`)}</th>
                    <td className="max-w-0 truncate px-3 py-1.5 font-mono text-fg" title={v.value} data-selectable>
                      {v.value}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Panel>

        <Panel icon={<ClipboardCopy />} title={t('copy.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          <p className="text-xs text-fg-muted">{info && size > 0 ? t('copy.selection', { count: selectionLength }) : t('copy.nothing')}</p>
          <DropdownMenu
            trigger={
              <Button size="sm" block disabled={!info || size === 0}>
                <ClipboardCopy />
                {t('copy.as')}
              </Button>
            }
            items={FORMATS.map((format) => ({ key: format, label: t(`copy.formats.${format}`), onSelect: () => void copyAs(format) }))}
          />
          <p className="text-[11px] leading-snug text-fg-subtle">{t('copy.hint')}</p>
        </Panel>
      </div>
    </div>
  )
}

export default HexViewer
