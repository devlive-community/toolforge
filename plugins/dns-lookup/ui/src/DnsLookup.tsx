import { useState } from 'react'
import { Badge, Button, Empty, Input, Panel, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Globe, Info, Search, Server, TriangleAlert } from 'lucide-react'
import type { Catalog, Report, ServerResult } from './types'

function Chip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        'rounded-control border px-2.5 py-1 text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
        active ? 'border-primary bg-primary-soft font-medium text-primary-fg' : 'border-border text-fg-muted hover:bg-hover hover:text-fg',
      )}
    >
      {children}
    </button>
  )
}

export function DnsLookup() {
  const { t, call, errorMessage } = usePlugin()
  const [name, setName] = useState('github.com')
  const [recordType, setRecordType] = useState('A')
  const [servers, setServers] = useState<string[]>(['system'])
  const [custom, setCustom] = useState('')
  const [report, setReport] = useState<Report | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const catalog = useDebouncedCall<Catalog>('servers', {}, [])
  const types = catalog.result?.types ?? ['A']
  const publicServers = catalog.result?.public ?? []

  const toggle = (id: string) => setServers((prev) => (prev.includes(id) ? prev.filter((s) => s !== id) : [...prev, id]))
  const selected = [...servers, ...custom.split(/[\s,]+/).filter(Boolean)]

  const lookup = async () => {
    if (!name.trim() || selected.length === 0 || busy) return
    setBusy(true)
    setError(null)
    try {
      setReport(await call<Report>('lookup', { name, recordType, servers: selected }))
    } catch (reason) {
      setError(reason)
      setReport(null)
    } finally {
      setBusy(false)
    }
  }

  const serverLabel = (result: ServerResult) => {
    if (result.server === 'system') return t('servers.system')
    const known = publicServers.find((s) => s.id === result.server)
    return known ? t(`servers.${known.id}`) : result.server
  }
  const fakeIp = report?.results.some((r) => r.answers.some((a) => a.fakeIp))

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <form
        className="space-y-3 rounded-card border border-border bg-surface p-4 shadow-card"
        onSubmit={(event) => {
          event.preventDefault()
          lookup()
        }}
      >
        <div className="flex items-center gap-2">
          <Input
            value={name}
            onChange={(event) => setName(event.target.value)}
            size="lg"
            className="font-mono"
            wrapperClassName="flex-1"
            leading={<Globe />}
            placeholder={t('input.placeholder')}
            aria-label={t('input.name')}
            spellCheck={false}
          />
          <Button type="submit" variant="primary" size="lg" loading={busy} disabled={!name.trim() || selected.length === 0}>
            {!busy && <Search />}
            {t('input.lookup')}
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="mr-1 w-16 text-xs text-fg-subtle">{t('input.type')}</span>
          {types.map((type) => (
            <Chip key={type} active={recordType === type} onClick={() => setRecordType(type)}>
              {type}
            </Chip>
          ))}
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="mr-1 w-16 text-xs text-fg-subtle">{t('input.servers')}</span>
          <Chip active={servers.includes('system')} onClick={() => toggle('system')}>
            {t('servers.system')}
          </Chip>
          {publicServers.map((server) => (
            <Chip key={server.id} active={servers.includes(server.id)} onClick={() => toggle(server.id)}>
              {`${t(`servers.${server.id}`)} ${server.address}`}
            </Chip>
          ))}
          <Input
            size="sm"
            value={custom}
            onChange={(event) => setCustom(event.target.value)}
            className="font-mono"
            wrapperClassName="w-44"
            placeholder={t('input.custom')}
            aria-label={t('input.custom')}
            spellCheck={false}
          />
        </div>
        {recordType === 'PTR' && <p className="text-xs text-fg-subtle">{t('input.ptrHint')}</p>}
      </form>

      <div className="min-h-0 overflow-auto">
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !report ? (
          <Empty icon={<Globe />} title={t('results.empty')} description={t('results.emptyHint')} />
        ) : (
          <div className="space-y-3">
            {fakeIp && (
              <p className="flex items-start gap-2 rounded-card border border-warning/30 bg-warning-soft px-3 py-2 text-xs text-warning">
                <Info className="mt-px size-3.5 shrink-0" />
                {t('results.fakeIp')}
              </p>
            )}
            <div className="grid gap-3 xl:grid-cols-2">
              {report.results.map((result) => (
                <Panel
                  key={result.server}
                  icon={<Server />}
                  title={serverLabel(result)}
                  extra={
                    <>
                      {result.address && <span className="font-mono text-xs text-fg-subtle">{result.address}</span>}
                      <Badge variant={result.error ? 'danger' : 'success'}>{t('results.ms', { ms: result.ms })}</Badge>
                    </>
                  }
                  bodyClassName="p-1"
                >
                  {result.error ? (
                    <p className="flex items-start gap-2 px-3 py-3 text-xs text-danger">
                      <TriangleAlert className="mt-px size-3.5 shrink-0" />
                      {errorMessage(result.error)}
                    </p>
                  ) : (
                    <table className="w-full text-left text-xs">
                      <tbody className="font-mono">
                        {result.answers.map((answer, index) => (
                          <tr key={index} className="group border-b border-border last:border-0 hover:bg-hover">
                            <td className="w-14 px-3 py-2 font-sans font-semibold text-fg-muted">{answer.recordType}</td>
                            <td className={cn('px-3 py-2 break-all', answer.fakeIp ? 'text-warning' : 'text-fg')} data-selectable>
                              {answer.value}
                            </td>
                            <td className="w-24 px-3 py-2 text-right text-fg-subtle tabular-nums">{t('results.ttl', { ttl: answer.ttl })}</td>
                            <td className="w-9 pr-2">
                              <CopyButton text={answer.value} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  )}
                </Panel>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
