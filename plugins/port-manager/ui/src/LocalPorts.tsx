import { useEffect, useState } from 'react'
import { Badge, Button, Checkbox, Empty, Input, Modal, Panel, SegmentedControl, Spinner, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, formatBytes, host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FolderSearch, RefreshCw, Search, Server, Skull } from 'lucide-react'
import type { Listing, ProcessInfo, Socket } from './types'

type ProtocolFilter = 'all' | 'tcp' | 'udp'
const REFRESH_MS = 3000

/** 0.0.0.0 / :: 表示监听所有网卡 */
const isWildcard = (address: string) => address === '0.0.0.0' || address === '::' || address === '*'

export function LocalPorts() {
  const { t, call, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const [protocol, setProtocol] = useState<ProtocolFilter>('all')
  const [connections, setConnections] = useState(false)
  const [auto, setAuto] = useState(false)
  const [tick, setTick] = useState(0)
  const [target, setTarget] = useState<{ process: ProcessInfo; port: number } | null>(null)
  const [force, setForce] = useState(false)
  const [busy, setBusy] = useState(false)

  const args = { query, connections, protocol: protocol === 'all' ? null : protocol }
  const { result, error, pending } = useDebouncedCall<Listing>('list', args, [query, connections, protocol, tick])

  useEffect(() => {
    if (!auto) return
    const timer = setInterval(() => setTick((n) => n + 1), REFRESH_MS)
    return () => clearInterval(timer)
  }, [auto])

  const processes = new Map((result?.processes ?? []).map((p) => [p.pid, p]))

  const terminate = async () => {
    if (!target) return
    setBusy(true)
    try {
      await call('terminate', { pid: target.process.pid, force })
      toast.success(t('terminate.done', { name: target.process.name, pid: target.process.pid }))
      setTarget(null)
      // 进程退出需要一点时间，稍后刷新
      setTimeout(() => setTick((n) => n + 1), 600)
    } catch (err) {
      toast.error(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  const reveal = (path: string) => host.revealPath(path).catch((err) => toast.error(errorMessage(err)))

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <Input
          size="sm"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t('local.search')}
          aria-label={t('local.search')}
          leading={<Search />}
          wrapperClassName="w-72"
        />
        <SegmentedControl<ProtocolFilter>
          size="sm"
          value={protocol}
          onValueChange={setProtocol}
          aria-label={t('local.protocol')}
          options={[
            { value: 'all', label: t('local.all') },
            { value: 'tcp', label: 'TCP' },
            { value: 'udp', label: 'UDP' },
          ]}
        />
        <label className="flex items-center gap-1.5 text-xs text-fg-muted">
          <Switch size="sm" checked={connections} onCheckedChange={setConnections} aria-label={t('local.connections')} />
          {t('local.connections')}
        </label>
        <label className="flex items-center gap-1.5 text-xs text-fg-muted">
          <Switch size="sm" checked={auto} onCheckedChange={setAuto} aria-label={t('local.auto')} />
          {t('local.auto')}
        </label>
        <div className="ml-auto flex items-center gap-2">
          {pending && <Spinner className="size-3.5 text-fg-subtle" />}
          {result && <span className="text-xs text-fg-muted tabular-nums">{t('local.count', { shown: result.sockets.length, total: result.total })}</span>}
          <Button size="sm" onClick={() => setTick((n) => n + 1)}>
            <RefreshCw />
            {t('local.refresh')}
          </Button>
        </div>
      </section>

      <Panel icon={<Server />} title={t('local.title')} bodyClassName="overflow-auto">
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <div className="flex h-40 items-center justify-center">
            <Spinner />
          </div>
        ) : result.sockets.length === 0 ? (
          <Empty icon={<Search />} title={t('local.empty')} />
        ) : (
          <table className="w-full text-left text-[12.5px]">
            <thead className="sticky top-0 z-10 bg-surface-2 text-xs text-fg-muted">
              <tr>
                <th className="px-3 py-2 font-semibold">{t('local.port')}</th>
                <th className="px-3 py-2 font-semibold">{t('local.address')}</th>
                <th className="px-3 py-2 font-semibold">{t('local.state')}</th>
                <th className="px-3 py-2 font-semibold">{t('local.process')}</th>
                <th className="px-3 py-2 font-semibold">{t('local.user')}</th>
                <th className="px-3 py-2 text-right font-semibold">{t('local.memory')}</th>
                <th className="w-24 px-3 py-2" />
              </tr>
            </thead>
            <tbody>
              {result.sockets.map((socket, index) => (
                <SocketRow
                  key={`${socket.protocol}-${socket.localAddr}-${socket.port}-${socket.remotePort ?? ''}-${index}`}
                  socket={socket}
                  process={socket.pids.map((pid) => processes.get(pid)).find(Boolean) ?? null}
                  onTerminate={(process) => {
                    setForce(false)
                    setTarget({ process, port: socket.port })
                  }}
                  onReveal={reveal}
                />
              ))}
            </tbody>
          </table>
        )}
      </Panel>

      <Modal
        open={target !== null}
        onOpenChange={(open) => !open && setTarget(null)}
        size="sm"
        title={t('terminate.title')}
        description={target ? t('terminate.description', { name: target.process.name, pid: target.process.pid, port: target.port }) : undefined}
        footer={
          <>
            <Button onClick={() => setTarget(null)}>{t('terminate.cancel')}</Button>
            <Button variant="danger" loading={busy} onClick={terminate}>
              {t('terminate.confirm')}
            </Button>
          </>
        }
      >
        {target && (
          <div className="space-y-3 px-5 pb-4">
            {target.process.command && (
              <code className="block max-h-24 overflow-auto rounded-control bg-surface-2 p-2 font-mono text-[11.5px] break-all text-fg-muted" data-selectable>
                {target.process.command}
              </code>
            )}
            <Checkbox checked={force} onCheckedChange={setForce}>
              <span className="text-[13px]">{t('terminate.force')}</span>
            </Checkbox>
            <p className="text-xs text-fg-subtle">{t('terminate.hint')}</p>
          </div>
        )}
      </Modal>
    </div>
  )
}

interface SocketRowProps {
  socket: Socket
  process: ProcessInfo | null
  onTerminate: (process: ProcessInfo) => void
  onReveal: (path: string) => void
}

function SocketRow({ socket, process, onTerminate, onReveal }: SocketRowProps) {
  const { t } = usePlugin()
  const listening = socket.state === 'listen' || socket.state === 'bound'
  return (
    <tr className="group border-t border-border hover:bg-hover">
      <td className="px-3 py-2 align-top">
        <div className="flex items-center gap-1.5">
          <span className="font-mono text-[13px] font-semibold text-fg tabular-nums" data-selectable>
            {socket.port}
          </span>
          <Badge variant={socket.protocol === 'tcp' ? 'info' : 'neutral'}>{socket.protocol.toUpperCase()}</Badge>
        </div>
      </td>
      <td className="px-3 py-2 align-top">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="font-mono text-fg" data-selectable>
            {socket.localAddr}
          </span>
          {listening &&
            (socket.loopback ? (
              <Tooltip content={t('local.loopbackHint')}>
                <Badge>{t('local.loopback')}</Badge>
              </Tooltip>
            ) : isWildcard(socket.localAddr) ? (
              <Tooltip content={t('local.wildcardHint')}>
                <Badge variant="warning">{t('local.wildcard')}</Badge>
              </Tooltip>
            ) : null)}
        </div>
        {socket.remoteAddr && (
          <p className="mt-0.5 font-mono text-[11.5px] text-fg-subtle" data-selectable>
            {`→ ${socket.remoteAddr}:${socket.remotePort}`}
          </p>
        )}
      </td>
      <td className="px-3 py-2 align-top">
        <span className={cn('text-xs', listening ? 'text-success' : 'text-fg-muted')}>{t(`states.${socket.state}`, { defaultValue: socket.state })}</span>
      </td>
      <td className="px-3 py-2 align-top">
        {process ? (
          <div className="min-w-0" title={process.command ?? process.exe ?? undefined}>
            <p className="truncate font-medium text-fg">{process.name}</p>
            <p className="font-mono text-[11.5px] text-fg-subtle tabular-nums">{`PID ${process.pid}`}</p>
          </div>
        ) : socket.pids.length > 0 ? (
          <span className="font-mono text-fg-subtle">{`PID ${socket.pids.join(', ')}`}</span>
        ) : (
          <Tooltip content={t('local.unknownHint')}>
            <span className="text-fg-subtle">{t('local.unknown')}</span>
          </Tooltip>
        )}
      </td>
      <td className="px-3 py-2 align-top text-fg-muted">{process?.user ?? ''}</td>
      <td className="px-3 py-2 text-right align-top text-fg-muted tabular-nums">{process ? formatBytes(process.memory) : ''}</td>
      <td className="px-3 py-1.5 align-top">
        <div className="flex justify-end gap-0.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
          <CopyButton text={String(socket.port)} />
          {process?.exe && (
            <Tooltip content={t('local.reveal')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('local.reveal')} onClick={() => onReveal(process.exe!)}>
                <FolderSearch />
              </Button>
            </Tooltip>
          )}
          {process && (
            <Tooltip content={t('terminate.action')}>
              <Button size="icon-sm" variant="ghost" aria-label={t('terminate.action')} className="hover:text-danger" onClick={() => onTerminate(process)}>
                <Skull />
              </Button>
            </Tooltip>
          )}
        </div>
      </td>
    </tr>
  )
}
