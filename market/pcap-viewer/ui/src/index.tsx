import { useState } from 'react'
import { Button, Input, Panel, Spinner } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Network, Play } from 'lucide-react'

interface Packet {
  index: number
  tsSec: number
  tsUsec: number
  src: string
  dst: string
  protocol: string
  srcPort: number | null
  dstPort: number | null
  tcpFlags: string | null
  capturedLen: number
  originalLen: number
}
interface Conversation {
  key: string
  packets: number
  bytes: number
  firstSec: number
  lastSec: number
}
interface ParseOut {
  linkType: string
  packets: Packet[]
  conversations: Conversation[]
  total: number
  truncatedCount: number
}

export default function PcapViewer() {
  const { t, errorMessage } = usePlugin()
  const [path, setPath] = useState('')
  const [limit, setLimit] = useState(2000) // tf-allow: 默认值
  const args = path ? { path, limit } : null
  const parse = useDebouncedCall<ParseOut>(args ? 'parse' : null, args, [path, limit])

  const pick = async () => {
    const picked = await host.dialog.openFile([{ name: 'PCAP', extensions: ['pcap', 'cap'] }])
    if (picked) setPath(picked)
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<Network />} title={t('ui.pick')} bodyClassName="space-y-2.5 p-3">
        <div className="flex gap-2">
          <Button onClick={() => void pick()}>{t('ui.pick_file')}</Button>
          <Input value={path} onChange={(event) => setPath(event.target.value)} placeholder={t('api.parse.path')} aria-label={t('api.parse.path')} wrapperClassName="min-w-0 flex-1" />
        </div>
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('api.parse.limit')}
          <input type="range" min={100} max={5000} step={100} value={limit} onChange={(event) => setLimit(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('api.parse.limit')} />
          <span className="w-14 text-right tabular-nums">{limit}</span>
        </label>
        {parse.result && (
          <p className="text-xs text-fg-muted">
            {t('ui.link')}: {parse.result.linkType} · {t('conversations.packets')}: {parse.result.total}
            {parse.result.truncatedCount > 0 && ` · ${t('ui.truncated', { count: parse.result.truncatedCount })}`}
          </p>
        )}
      </Panel>
      <Panel icon={<Play />} title={t('table.index')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto p-3" extra={parse.pending && <Spinner className="size-3.5" />}>
        {parse.error ? (
          <p className="text-xs text-danger">{errorMessage(parse.error)}</p>
        ) : (
          parse.result && (
            <>
              {parse.result.conversations.length > 0 && (
                <div className="mb-3 space-y-1">
                  <p className="text-xs font-medium text-fg-muted">{t('conversations.title')}</p>
                  {parse.result.conversations.slice(0, 10).map((conversation) => (
                    <p key={conversation.key} className="truncate font-mono text-[11px] text-fg-muted">
                      {conversation.key}{' — '}{conversation.packets}{' '}{t('conversations.packets')}{' / '}{conversation.bytes}{' '}{t('conversations.bytes')}
                    </p>
                  ))}
                </div>
              )}
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-fg-subtle">
                    <th className="py-1 font-medium">{t('table.index')}</th>
                    <th className="py-1 font-medium">{t('table.time')}</th>
                    <th className="py-1 font-medium">{t('table.src')}</th>
                    <th className="py-1 font-medium">{t('table.dst')}</th>
                    <th className="py-1 font-medium">{t('table.protocol')}</th>
                    <th className="py-1 text-right font-medium">{t('table.len')}</th>
                  </tr>
                </thead>
                <tbody className="font-mono">
                  {parse.result.packets.map((packet) => (
                    <tr key={packet.index} className="border-t border-border">
                      <td className="py-1 tabular-nums">{packet.index}</td>
                      <td className="py-1 tabular-nums">{packet.tsSec}.{String(packet.tsUsec).padStart(6, '0')}</td>
                      <td className="py-1">{packet.src}{packet.srcPort !== null ? `:${packet.srcPort}` : ''}</td>
                      <td className="py-1">{packet.dst}{packet.dstPort !== null ? `:${packet.dstPort}` : ''}</td>
                      <td className="py-1">
                        {packet.protocol}
                        {packet.tcpFlags && <span className="ml-1 text-fg-subtle">[{packet.tcpFlags}]</span>}
                      </td>
                      <td className="py-1 text-right tabular-nums">{packet.capturedLen}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )
        )}
      </Panel>
    </div>
  )
}
