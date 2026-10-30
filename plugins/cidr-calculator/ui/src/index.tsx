import { useState } from 'react'
import { Button, Input, Panel } from '@toolforge/ui'
import { CopyButton, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Calculator, Network, Search } from 'lucide-react'

type NetworkInfo = { cidr: string; version: number; network: string; broadcast: string; mask: string; wildcard: string; firstHost: string; lastHost: string; addresses: string; usableHosts: string; prefix: number }
type SplitReport = { total: string; page: number; hasNext: boolean; subnets: NetworkInfo[] }

export default function CidrCalculator() {
  const { t, call, errorMessage } = usePlugin()
  const [input, setInput] = useState('192.168.1.0/24')
  useLaunchInput(setInput)
  const [result, setResult] = useState<NetworkInfo | null>(null)
  const [prefix, setPrefix] = useState('26')
  const [plan, setPlan] = useState<SplitReport | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [planError, setPlanError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)
  const [planning, setPlanning] = useState(false)
  const calculate = async () => {
    setBusy(true); setError(null); setResult(null); setPlan(null); setPlanError(null)
    try {
      const value = await call<NetworkInfo>('calculate', { cidr: input })
      setResult(value); setPrefix(String(Math.min(value.prefix + 2, value.version === 4 ? 32 : 128)))
    } catch (reason) { setError(reason) } finally { setBusy(false) }
  }
  const split = async (page = 0) => {
    if (!result) return
    setPlanning(true); setPlanError(null); setPlan(null)
    try { setPlan(await call<SplitReport>('split', { cidr: result.cidr, prefix: Number(prefix), page })) }
    catch (reason) { setPlanError(reason) } finally { setPlanning(false) }
  }
  const fields: [string, string][] = result ? [
    ['network', result.network], [result.version === 4 ? 'broadcast' : 'lastAddress', result.broadcast], ['mask', result.mask], ['wildcard', result.wildcard],
    ['firstHost', result.firstHost], ['lastHost', result.lastHost], ['addresses', BigInt(result.addresses).toLocaleString()], ['usableHosts', BigInt(result.usableHosts).toLocaleString()],
  ] : []
  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <form className="flex flex-wrap items-end gap-2 rounded-card border border-border bg-surface p-4 shadow-card" onSubmit={(e) => { e.preventDefault(); void calculate() }}>
        <label className="min-w-48 flex-1 space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('input.label')}</span>
          <Input value={input} onChange={(e) => { setInput(e.target.value); setResult(null); setPlan(null); setError(null) }} disabled={busy || planning} size="lg" className="font-mono" placeholder="192.168.1.10/24" aria-label={t('input.label')} spellCheck={false} />
        </label>
        <Button type="submit" variant="primary" size="lg" loading={busy} disabled={!input.trim() || planning}><Search />{t('input.calculate')}</Button>
        <p className="w-full text-xs text-fg-subtle">{t('input.hint')}</p>
      </form>
      {error != null && <p role="alert" className="rounded-card border border-danger/30 bg-surface p-3 text-sm text-danger">{errorMessage(error)}</p>}
      {!result && !error && <div className="flex flex-1 flex-col items-center justify-center gap-2 p-8 text-center text-fg-subtle"><Calculator className="size-8" /><p className="text-sm">{t('empty')}</p></div>}
      {result && <div className="space-y-3">
        <Panel icon={<Calculator />} title={t('result.title')} extra={<span className="font-mono text-xs text-primary-fg">{result.cidr}</span>} bodyClassName="p-3">
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            {fields.map(([key, value]) => <div key={key} className="group min-w-0 rounded-control border border-border p-3"><p className="text-xs text-fg-subtle">{t(`result.${key}`)}</p><div className="mt-1 flex items-center justify-between gap-1"><span className="break-all font-mono text-sm text-fg" data-selectable>{value}</span><CopyButton text={value} /></div></div>)}
          </div>
          <p className="mt-3 text-xs text-fg-subtle">{t(result.version === 4 ? 'result.hostHint' : 'result.ipv6Hint')}</p>
        </Panel>
        <Panel icon={<Network />} title={t('plan.title')} bodyClassName="space-y-3 p-3">
          <p className="text-xs text-fg-muted">{t('plan.hint', { cidr: result.cidr, prefix: result.prefix, max: result.version === 4 ? 32 : 128 })}</p>
          <form className="flex flex-wrap items-end gap-2" onSubmit={(e) => { e.preventDefault(); void split() }}>
            <label className="space-y-1.5"><span className="text-xs font-medium text-fg-muted">{t('plan.prefix')}</span><Input type="number" min={result.prefix} max={result.version === 4 ? 32 : 128} step={1} value={prefix} disabled={planning} onChange={(e) => { setPrefix(e.target.value); setPlan(null); setPlanError(null) }} aria-label={t('plan.prefix')} wrapperClassName="w-32" /></label>
            <Button type="submit" loading={planning} disabled={!/^\d+$/.test(prefix) || Number(prefix) < result.prefix || Number(prefix) > (result.version === 4 ? 32 : 128)}>{t('plan.split')}</Button>
          </form>
          {planError != null && <p role="alert" className="text-xs text-danger">{errorMessage(planError)}</p>}
          {plan && <>
            <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-fg-muted"><span>{t('plan.count', { total: BigInt(plan.total).toLocaleString() })}</span><div className="flex items-center gap-2"><Button size="sm" disabled={planning || plan.page === 0} onClick={() => void split(plan.page - 1)}>{t('plan.previous')}</Button><span>{t('plan.page', { page: plan.page + 1 })}</span><Button size="sm" disabled={planning || !plan.hasNext} onClick={() => void split(plan.page + 1)}>{t('plan.next')}</Button></div></div>
            <div className="overflow-x-auto"><table className="w-full text-left text-xs"><thead className="border-b border-border text-fg-subtle"><tr>{['subnet', 'range', 'usableHosts'].map((key) => <th key={key} className="px-2 py-2 font-medium">{t(`plan.${key}`)}</th>)}</tr></thead><tbody>{plan.subnets.map((subnet) => <tr key={subnet.cidr} className="border-b border-border last:border-0"><td className="px-2 py-2 font-mono text-fg"><div className="flex items-center gap-1">{subnet.cidr}<CopyButton text={subnet.cidr} /></div></td><td className="px-2 py-2 font-mono text-fg-muted">{subnet.firstHost} – {subnet.lastHost}</td><td className="px-2 py-2 tabular-nums text-fg-muted">{BigInt(subnet.usableHosts).toLocaleString()}</td></tr>)}</tbody></table></div>
          </>}
        </Panel>
      </div>}
    </div>
  )
}
