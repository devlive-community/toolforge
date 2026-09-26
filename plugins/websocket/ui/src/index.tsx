import { useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Select, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowDownLeft, ArrowUpRight, Bookmark, ChevronDown, CircleDot, Eraser, History, Plug, PlugZap, Plus, Radio, Search, Send, Settings2, Trash2, X } from 'lucide-react'
import { DEFAULT_SETTINGS, type Direction, type Encoding, type Info, type Message, type Poll, type SendKind, type Settings, type Snippet, type State } from './types'

const POLL_MS = 150
const MAX_MESSAGES = 5000
const ROW_HEIGHT = 40
const MAX_HISTORY = 10

const newSession = () => (globalThis.crypto?.randomUUID?.() ?? `s${Date.now()}${Math.random()}`)
const clock = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit', fractionalSecondDigits: 3, hour12: false })

const STATE_BADGE: Record<State, 'neutral' | 'success' | 'warning' | 'info'> = { connecting: 'info', open: 'success', closing: 'warning', closed: 'neutral' }

function summary(message: Message): string {
  if (message.text !== null) return message.text.slice(0, 500)
  if (message.hex !== null) return message.hex
  return ''
}

function WebSocketClient() {
  const { t, call, errorMessage } = usePlugin()
  const [settings, setSettings] = usePluginState<Settings>('settings', DEFAULT_SETTINGS)
  const [snippets, setSnippets] = usePluginState<Snippet[]>('snippets', [])
  const task = useTask()
  const [session, setSession] = useState<string | null>(null)
  const [state, setState] = useState<State>('closed')
  const [info, setInfo] = useState<Info | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [filter, setFilter] = useState<'all' | Direction>('all')
  const [search, setSearch] = useState('')
  const [selected, setSelected] = useState<Message | null>(null)
  const [pretty, setPretty] = useState<string | null>(null)
  const [showOptions, setShowOptions] = useState(false)
  const [kind, setKind] = useState<SendKind>('text')
  const [encoding, setEncoding] = useState<Encoding>('text')
  const [payload, setPayload] = useState('')
  const [follow, setFollow] = useState(true)
  const lastSeq = useRef(0)
  const listRef = useRef<HTMLDivElement>(null)
  const update = (patch: Partial<Settings>) => setSettings({ ...settings, ...patch })

  // 轮询会话中的新消息；连接结束后再取最后一次
  useEffect(() => {
    if (!session) return
    let alive = true
    let finished = false
    const tick = () => {
      call<Poll>('messages', { session, after: lastSeq.current })
        .then((poll) => {
          if (!alive) return
          if (poll.messages.length > 0) {
            lastSeq.current = poll.last
            setMessages((prev) => {
              const next = prev.concat(poll.messages)
              return next.length > MAX_MESSAGES ? next.slice(next.length - MAX_MESSAGES) : next
            })
          }
          setState(poll.state)
          setInfo(poll.info)
          if (poll.state === 'closed') finished = true
        })
        // 任务刚启动时会话可能还未创建
        .catch(() => {})
    }
    tick()
    const timer = setInterval(() => {
      if (finished) {
        clearInterval(timer)
        return
      }
      tick()
    }, POLL_MS)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [session, call])

  useEffect(() => {
    if (task.status === 'failed' && task.error && task.error.code !== 'task.cancelled') toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const connect = () => {
    const url = settings.url.trim()
    if (!url) return
    const id = newSession()
    lastSeq.current = 0
    setMessages([])
    setSelected(null)
    setState('connecting')
    setSession(id)
    update({ history: [url, ...settings.history.filter((h) => h !== url)].slice(0, MAX_HISTORY) })
    const protocols = settings.protocols.split(',').map((p) => p.trim()).filter(Boolean)
    task.start('connect', { session: id, url, headers: settings.headers.filter(([k]) => k.trim()), protocols })
  }
  const disconnect = () => {
    if (!session) return
    call('close', { session, code: 1000, reason: '' }).catch(() => task.cancel())
  }

  const send = async (override?: Snippet) => {
    if (!session) return
    const body = override ?? { name: '', kind, encoding, payload }
    try {
      await call('send', { session, kind: body.kind, payload: body.payload, encoding: body.kind === 'text' ? 'text' : body.encoding })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const saveSnippet = () => {
    const name = payload.trim().split('\n')[0].slice(0, 40) || t('snippets.untitled')
    setSnippets([...snippets.filter((s) => s.name !== name), { name, kind, encoding, payload }].slice(-30))
    toast.success(t('snippets.saved'))
  }

  const select = (message: Message) => {
    setSelected(message)
    setPretty(null)
    if (message.text && (message.text.trimStart().startsWith('{') || message.text.trimStart().startsWith('['))) {
      call<string | null>('pretty', { text: message.text }).then(setPretty).catch(() => {})
    }
  }

  const needle = search.trim().toLowerCase()
  const shown = useMemo(
    () =>
      messages.filter(
        (m) => (filter === 'all' || m.direction === filter) && (!needle || (m.text ?? m.hex ?? m.code ?? '').toLowerCase().includes(needle)),
      ),
    [messages, filter, needle],
  )
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({ count: shown.length, getScrollElement: () => listRef.current, estimateSize: () => ROW_HEIGHT, overscan: 16 })
  useEffect(() => {
    if (follow && shown.length > 0) virtualizer.scrollToIndex(shown.length - 1, { align: 'end' })
  }, [follow, shown.length, virtualizer])

  const counts = useMemo(() => ({ in: messages.filter((m) => m.direction === 'in').length, out: messages.filter((m) => m.direction === 'out').length }), [messages])
  const connected = state === 'open'
  const busy = state === 'connecting' || state === 'closing'

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-col gap-2.5 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex items-center gap-2">
          <Input
            value={settings.url}
            onChange={(e) => update({ url: e.target.value })}
            onKeyDown={(e) => e.key === 'Enter' && !connected && !busy && connect()}
            placeholder="wss://example.com/socket"
            className="font-mono"
            spellCheck={false}
            disabled={connected || busy}
            wrapperClassName="flex-1"
            aria-label={t('connection.url')}
            leading={<Radio />}
            trailing={
              settings.history.length > 0 && !connected && !busy ? (
                <DropdownMenu
                  trigger={
                    <Button size="icon-sm" variant="ghost" aria-label={t('connection.history')}>
                      <History />
                    </Button>
                  }
                  items={settings.history.map((url) => ({ key: url, label: url, onSelect: () => update({ url }) }))}
                />
              ) : undefined
            }
          />
          <Badge variant={STATE_BADGE[state]}>
            <CircleDot className="size-3" />
            {t(`states.${state}`)}
          </Badge>
          <Tooltip content={t('connection.options')}>
            <Button size="icon-md" variant={showOptions ? 'primary' : 'ghost'} aria-pressed={showOptions} aria-label={t('connection.options')} onClick={() => setShowOptions(!showOptions)}>
              <Settings2 />
            </Button>
          </Tooltip>
          {connected || state === 'closing' ? (
            <Button onClick={disconnect} disabled={state === 'closing'}>
              <Plug />
              {t('connection.disconnect')}
            </Button>
          ) : (
            <Button variant="primary" onClick={connect} disabled={busy || !settings.url.trim()} loading={state === 'connecting'}>
              <PlugZap />
              {t('connection.connect')}
            </Button>
          )}
        </div>
        {showOptions && (
          <div className="grid gap-3 border-t border-border pt-3 md:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
            <div className="space-y-1.5">
              <p className="flex items-center justify-between text-xs font-medium text-fg-muted">
                {t('connection.headers')}
                <Button size="sm" variant="ghost" disabled={connected || busy} onClick={() => update({ headers: [...settings.headers, ['', '']] })}>
                  <Plus />
                  {t('connection.addHeader')}
                </Button>
              </p>
              {settings.headers.length === 0 && <p className="text-[11px] text-fg-subtle">{t('connection.noHeaders')}</p>}
              {settings.headers.map(([name, value], index) => (
                <div key={index} className="flex items-center gap-1.5">
                  <Input size="sm" value={name} placeholder="Authorization" disabled={connected || busy} className="font-mono" wrapperClassName="w-44" onChange={(e) => update({ headers: settings.headers.map((h, i) => (i === index ? [e.target.value, h[1]] : h)) })} aria-label={t('connection.headerName')} />
                  <Input size="sm" value={value} placeholder="Bearer …" disabled={connected || busy} className="font-mono" wrapperClassName="flex-1" onChange={(e) => update({ headers: settings.headers.map((h, i) => (i === index ? [h[0], e.target.value] : h)) })} aria-label={t('connection.headerValue')} />
                  <Button size="icon-sm" variant="ghost" disabled={connected || busy} aria-label={t('connection.removeHeader')} onClick={() => update({ headers: settings.headers.filter((_, i) => i !== index) })}>
                    <X />
                  </Button>
                </div>
              ))}
            </div>
            <div className="space-y-1.5">
              <p className="text-xs font-medium text-fg-muted">{t('connection.protocols')}</p>
              <Input size="sm" value={settings.protocols} onChange={(e) => update({ protocols: e.target.value })} placeholder="graphql-ws, chat" disabled={connected || busy} className="font-mono" aria-label={t('connection.protocols')} />
              {info?.status && (
                <p className="text-[11px] text-fg-subtle">
                  {t('connection.handshake', { status: info.status, ms: info.handshakeMs ?? 0, address: info.address ?? '' })}
                  {info.protocol && ` · ${t('connection.protocol', { protocol: info.protocol })}`}
                </p>
              )}
            </div>
          </div>
        )}
      </section>

      <div className={cn('grid min-h-0 flex-1 gap-3', selected ? 'grid-cols-[minmax(0,1fr)_380px]' : 'grid-cols-1')}>
        <div className="flex min-h-0 flex-col gap-3">
          <Panel
            title={t('messages.title')}
            extra={
              <span className="flex items-center gap-1.5 text-xs text-fg-muted tabular-nums">
                <ArrowDownLeft className="size-3.5 text-success" />
                {counts.in}
                <ArrowUpRight className="ml-1 size-3.5 text-info" />
                {counts.out}
              </span>
            }
            actions={
              <>
                <Input size="sm" value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t('messages.search')} leading={<Search />} wrapperClassName="w-44" aria-label={t('messages.search')} />
                <SegmentedControl
                  size="sm"
                  value={filter}
                  onValueChange={setFilter}
                  aria-label={t('messages.filter')}
                  options={(['all', 'in', 'out', 'event'] as const).map((value) => ({ value, label: t(`filters.${value}`) }))}
                />
                <Tooltip content={t('messages.clear')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t('messages.clear')}
                    onClick={() => {
                      setMessages([])
                      setSelected(null)
                      if (session) call('clear', { session }).catch(() => {})
                    }}
                  >
                    <Eraser />
                  </Button>
                </Tooltip>
              </>
            }
            className="min-h-0 flex-1"
            bodyClassName="p-0"
          >
            {shown.length === 0 ? (
              <Empty icon={<Radio />} title={messages.length === 0 ? t('messages.empty') : t('messages.noMatch')} description={messages.length === 0 ? t('messages.emptyHint') : undefined} />
            ) : (
              <div ref={listRef} className="h-full overflow-auto" onWheel={(e) => e.deltaY < 0 && setFollow(false)}>
                <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
                  {virtualizer.getVirtualItems().map((row) => {
                    const message = shown[row.index]
                    const Icon = message.direction === 'in' ? ArrowDownLeft : message.direction === 'out' ? ArrowUpRight : CircleDot
                    return (
                      <button
                        key={row.key}
                        type="button"
                        onClick={() => select(message)}
                        className={cn(
                          'absolute left-0 flex w-full items-center gap-2.5 border-b border-border px-3 text-left outline-none hover:bg-hover focus-visible:bg-hover',
                          selected?.seq === message.seq && 'bg-primary-soft hover:bg-primary-soft',
                        )}
                        style={{ top: row.start, height: ROW_HEIGHT }}
                      >
                        <Icon className={cn('size-4 shrink-0', message.direction === 'in' ? 'text-success' : message.direction === 'out' ? 'text-info' : message.kind === 'error' ? 'text-danger' : 'text-fg-subtle')} />
                        <span className="shrink-0 font-mono text-[11px] text-fg-subtle tabular-nums">{clock.format(message.ts)}</span>
                        {message.kind !== 'text' && <Badge variant={message.kind === 'error' ? 'danger' : 'neutral'}>{t(`kinds.${message.kind}`)}</Badge>}
                        <span className={cn('min-w-0 flex-1 truncate font-mono text-[12px]', message.direction === 'event' ? 'text-fg-muted' : 'text-fg')}>
                          {message.direction === 'event'
                            ? [message.code && (message.kind === 'close' ? t('messages.closeCode', { code: message.code }) : errorMessage({ code: message.code, params: {} })), message.text].filter(Boolean).join(' · ')
                            : summary(message)}
                        </span>
                        {message.direction !== 'event' && <span className="shrink-0 text-[11px] text-fg-subtle tabular-nums">{formatBytes(message.size)}</span>}
                      </button>
                    )
                  })}
                </div>
              </div>
            )}
          </Panel>

          <Panel
            title={
              <span className="flex items-center gap-2">
                <SegmentedControl<SendKind>
                  size="sm"
                  value={kind}
                  onValueChange={setKind}
                  aria-label={t('composer.kind')}
                  options={(['text', 'binary', 'ping'] as SendKind[]).map((value) => ({ value, label: t(`kinds.${value}`) }))}
                />
                {kind !== 'text' && (
                  <Select<Encoding>
                    size="sm"
                    value={encoding}
                    onValueChange={setEncoding}
                    aria-label={t('composer.encoding')}
                    options={(['text', 'hex', 'base64'] as Encoding[]).map((value) => ({ value, label: t(`encodings.${value}`) }))}
                  />
                )}
              </span>
            }
            actions={
              <>
                {snippets.length > 0 && (
                  <DropdownMenu
                    trigger={
                      <Button size="sm" variant="ghost">
                        <Bookmark />
                        {t('snippets.title')}
                        <ChevronDown />
                      </Button>
                    }
                    items={[
                      ...snippets.map((snippet) => ({
                        key: snippet.name,
                        label: snippet.name,
                        onSelect: () => {
                          setKind(snippet.kind)
                          setEncoding(snippet.encoding)
                          setPayload(snippet.payload)
                        },
                      })),
                      { key: '__clear', label: t('snippets.clear'), icon: <Trash2 />, onSelect: () => setSnippets([]) },
                    ]}
                  />
                )}
                <Button size="sm" variant="ghost" onClick={saveSnippet} disabled={!payload.trim()}>
                  <Bookmark />
                  {t('snippets.save')}
                </Button>
                <Button size="sm" variant="primary" onClick={() => send()} disabled={!connected || (kind !== 'ping' && !payload)}>
                  <Send />
                  {t('composer.send')}
                </Button>
              </>
            }
            className="h-44 shrink-0"
            bodyClassName="p-0"
          >
            <div
              className="h-full"
              // 捕获阶段拦截，避免编辑器先插入换行
              onKeyDownCapture={(e) => {
                if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
                  e.preventDefault()
                  e.stopPropagation()
                  send()
                }
              }}
            >
              <CodeEditor value={payload} onChange={setPayload} language="json" lineWrapping placeholder={t('composer.placeholder')} aria-label={t('composer.payload')} />
            </div>
          </Panel>
        </div>

        {selected && (
          <Panel
            title={t('detail.title', { seq: selected.seq })}
            extra={<Badge>{t(`kinds.${selected.kind}`)}</Badge>}
            actions={
              <>
                {(selected.text ?? selected.base64) && <CopyButton text={pretty ?? selected.text ?? selected.base64 ?? ''} />}
                <Button size="icon-sm" variant="ghost" aria-label={t('detail.close')} onClick={() => setSelected(null)}>
                  <X />
                </Button>
              </>
            }
            bodyClassName="space-y-3 overflow-auto p-3"
          >
            <p className="text-xs text-fg-muted tabular-nums">
              {`${clock.format(selected.ts)} · ${t(`filters.${selected.direction}`)} · ${formatBytes(selected.size)}`}
            </p>
            {(pretty ?? selected.text) !== null && (
              <pre className="font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
                {pretty ?? selected.text}
              </pre>
            )}
            {selected.hex && (
              <div className="space-y-1">
                <p className="text-xs font-medium text-fg-muted">{t('detail.hex')}</p>
                <pre className="font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
                  {selected.hex}
                </pre>
              </div>
            )}
            {selected.truncated && <p className="text-xs text-warning">{t('detail.truncated')}</p>}
          </Panel>
        )}
      </div>
    </div>
  )
}

export default WebSocketClient
