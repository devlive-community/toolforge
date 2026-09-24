import { useState } from 'react'
import { Badge, Empty, Input, NumberInput, Panel, Spinner, Switch } from '@toolforge/ui'
import { CopyButton, ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CircleCheck, CircleX, Info, Network, Search, Split } from 'lucide-react'
import type { Report } from './types'

const EXAMPLES = ['192.168.1.130/26', '10.0.0.1 255.255.0.0', '172.16.5.4/12', '2001:db8:abcd:12::1/48', 'fe80::1/64']

export function SubnetTool() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('192.168.1.130/26')
  const [splitOn, setSplitOn] = useState(false)
  const [split, setSplit] = useState(28)
  const [contains, setContains] = useState('')

  const trimmed = input.trim()
  const args = trimmed ? { input: trimmed, split: splitOn ? split : null, contains } : null
  const { result, error, pending } = useDebouncedCall<Report>(args ? 'calculate' : null, args, [trimmed, splitOn, split, contains])
  const splitError = error?.code === 'ip.invalid_split'
  const containsError = error?.params?.field === 'contains'
  const report = error ? null : result
  const maxPrefix = report?.version === 6 ? 128 : 32

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="space-y-3 rounded-card border border-border bg-surface p-4 shadow-card">
        <Input
          value={input}
          onChange={(event) => setInput(event.target.value)}
          size="lg"
          className="font-mono"
          leading={<Network />}
          trailing={pending && <Spinner className="size-3.5" />}
          invalid={!!error && !splitError && !containsError}
          placeholder={t('input.placeholder')}
          aria-label={t('input.label')}
        />
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-fg-subtle">{t('input.examples')}</span>
          {EXAMPLES.map((example) => (
            <button
              key={example}
              type="button"
              onClick={() => setInput(example)}
              className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
            >
              {example}
            </button>
          ))}
        </div>
        {error && !splitError && !containsError && <p className="text-xs text-danger">{errorMessage(error)}</p>}
      </section>

      <div className="grid min-h-0 grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<Info />}
          title={t('details.title')}
          extra={
            report && (
              <>
                <Badge variant="success">{t('details.version', { version: report.version })}</Badge>
                <Badge>{t(`kinds.${report.kind}`)}</Badge>
                {report.class && <Badge>{t('details.class', { class: report.class })}</Badge>}
              </>
            )
          }
          bodyClassName="overflow-auto py-1"
        >
          {!report ? (
            <Empty icon={<Network />} title={t('details.empty')} />
          ) : (
            <>
              <ValueRow label={t('details.cidr')} value={report.cidr} labelWidth="w-24" />
              <ValueRow label={t('details.network')} value={report.network} labelWidth="w-24" />
              {report.broadcast && <ValueRow label={t('details.broadcast')} value={report.broadcast} labelWidth="w-24" />}
              <ValueRow label={t('details.netmask')} value={report.netmask} labelWidth="w-24" />
              <ValueRow label={t('details.wildcard')} value={report.wildcard} labelWidth="w-24" />
              <ValueRow label={t('details.hosts')} value={`${report.firstHost} – ${report.lastHost}`} labelWidth="w-24" />
              <ValueRow label={t('details.total')} value={report.total} labelWidth="w-24" />
              <ValueRow label={t('details.usable')} value={report.usable} labelWidth="w-24" />
              <div className="mx-3 my-1 border-t border-border" />
              <ValueRow label={t('details.binary')} value={report.binary} labelWidth="w-24" />
              <ValueRow label={t('details.hex')} value={report.hex} labelWidth="w-24" />
              <ValueRow label={t('details.integer')} value={report.integer} labelWidth="w-24" />
              <ValueRow label={t('details.expanded')} value={report.expanded} labelWidth="w-24" />
              <ValueRow label={t('details.reverseDns')} value={report.reverseDns} labelWidth="w-24" />
            </>
          )}
        </Panel>

        <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
          <Panel icon={<Search />} title={t('contains.title')} bodyClassName="space-y-2 p-3">
            <Input
              value={contains}
              onChange={(event) => setContains(event.target.value)}
              className="font-mono"
              invalid={containsError}
              placeholder={t('contains.placeholder')}
              aria-label={t('contains.title')}
            />
            {containsError && error ? (
              <p className="text-xs text-danger">{errorMessage(error)}</p>
            ) : (
              report?.membership && (
                <p className={report.membership.inside ? 'flex items-center gap-1.5 text-xs text-success' : 'flex items-center gap-1.5 text-xs text-warning'}>
                  {report.membership.inside ? <CircleCheck className="size-3.5" /> : <CircleX className="size-3.5" />}
                  {t(report.membership.inside ? 'contains.inside' : 'contains.outside', { address: report.membership.address, cidr: report.cidr })}
                </p>
              )
            )}
          </Panel>

          <Panel
            icon={<Split />}
            title={t('split.title')}
            extra={report?.subnetCount && <Badge>{t('split.count', { count: report.subnetCount })}</Badge>}
            actions={
              <>
                {splitOn && (
                  <NumberInput size="sm" value={split} onValueChange={setSplit} min={0} max={maxPrefix} className="w-28" aria-label={t('split.prefix')} />
                )}
                <Switch size="sm" checked={splitOn} onCheckedChange={setSplitOn} aria-label={t('split.title')} />
              </>
            }
            bodyClassName="overflow-auto p-2"
          >
            {!splitOn ? (
              <p className="p-3 text-xs text-fg-subtle">{t('split.hint')}</p>
            ) : splitError && error ? (
              <p className="p-3 text-xs text-danger">{errorMessage(error)}</p>
            ) : report && report.subnets.length > 0 ? (
              <>
                <ul className="font-mono text-[12.5px]">
                  {report.subnets.map((subnet) => (
                    <li key={subnet.cidr} className="group flex items-center gap-3 rounded-control px-3 py-1.5 hover:bg-hover" data-selectable>
                      <span className="w-48 shrink-0 text-fg">{subnet.cidr}</span>
                      <span className="min-w-0 flex-1 truncate text-fg-muted">{`${subnet.first} – ${subnet.last}`}</span>
                      <CopyButton text={subnet.cidr} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
                    </li>
                  ))}
                </ul>
                {report.subnetCount && Number(report.subnetCount) > report.subnets.length && (
                  <p className="px-3 py-2 text-xs text-fg-subtle">{t('split.truncated', { shown: report.subnets.length })}</p>
                )}
              </>
            ) : (
              <p className="p-3 text-xs text-fg-subtle">{t('split.hint')}</p>
            )}
          </Panel>
        </div>
      </div>
    </div>
  )
}
