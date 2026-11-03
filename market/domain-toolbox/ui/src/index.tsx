import { useState } from 'react'
import { Badge, Checkbox, Input, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Globe, Mail, ShieldCheck, Wand2 } from 'lucide-react'

interface Candidate {
  domain: string
  price: string
}
interface CandidateOut {
  candidates: Candidate[]
  total: number
}
interface DnsRecord {
  type: string
  name: string
  value: string
  ttl: number
  purpose: string
}
interface DnsOut {
  scenario: string
  domain: string
  records: DnsRecord[]
}
interface SpfOut {
  valid: boolean
  mechanisms: string[]
  includes: string[]
  policy: string
  issues: string[]
}
interface DmarcOut {
  valid: boolean
  policy: string
  reports: string[]
  percentage: number | null
  issues: string[]
}

export default function DomainToolbox() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'candidates' | 'dns' | 'email'>('candidates')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('tab.label')}
        options={[
          { value: 'candidates', label: t('tab.candidates') },
          { value: 'dns', label: t('tab.dns') },
          { value: 'email', label: t('tab.email') },
        ]}
      />
      {tab === 'candidates' ? <CandidatesTab /> : tab === 'dns' ? <DnsTab /> : <EmailTab />}
    </div>
  )
}

function CandidatesTab() {
  const { t, errorMessage } = usePlugin()
  const [brands, setBrands] = useState('acme') // tf-allow: 默认值
  const [tlds, setTlds] = useState('com,io,dev') // tf-allow: 默认值
  const [hyphens, setHyphens] = useState(true)

  const brandList = brands.split(',').map((b) => b.trim()).filter(Boolean)
  const tldList = tlds.split(',').map((b) => b.trim()).filter(Boolean)
  const args = brandList.length > 0 ? { brands: brandList, tlds: tldList, hyphens } : null
  const { result, error, pending } = useDebouncedCall<CandidateOut>(args ? 'candidates' : null, args, [brands, tlds, hyphens])

  return (
    <Panel icon={<Wand2 />} title={t('tab.candidates')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-2.5 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <Input value={brands} onChange={(event) => setBrands(event.target.value)} placeholder={t('api.candidates.brands')} aria-label={t('api.candidates.brands')} />
      <Input value={tlds} onChange={(event) => setTlds(event.target.value)} placeholder={t('api.candidates.tlds')} aria-label={t('api.candidates.tlds')} />
      <Checkbox checked={hyphens} onCheckedChange={(checked) => setHyphens(checked === true)} >{t('api.candidates.hyphens')}</Checkbox>
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2">
            {result.candidates.map((candidate) => (
              <div key={candidate.domain} className="flex items-center justify-between rounded-control border border-border px-2.5 py-1.5">
                <span className="font-mono text-xs text-fg">{candidate.domain}</span>
                <Badge variant={candidate.price === 'standard' ? 'success' : 'neutral'}>{t(`prices.${candidate.price}`)}</Badge>
              </div>
            ))}
          </div>
        )
      )}
    </Panel>
  )
}

function DnsTab() {
  const { t, errorMessage } = usePlugin()
  const [scenario, setScenario] = useState<'website' | 'cdn'>('website')
  const [domain, setDomain] = useState('acme.com') // tf-allow: 默认值
  const [ip, setIp] = useState('203.0.113.10') // tf-allow: 默认值
  const [target, setTarget] = useState('') // tf-allow: 默认值

  const args =
    domain.trim() && (scenario === 'cdn' ? target.trim() : true)
      ? { scenario, domain: domain.trim(), ip: scenario === 'website' ? ip.trim() || undefined : undefined, target: target.trim() || undefined }
      : null
  const { result, error, pending } = useDebouncedCall<DnsOut>(args ? 'dns' : null, args, [scenario, domain, ip, target])

  return (
    <Panel icon={<Globe />} title={t('tab.dns')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-2.5 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <SegmentedControl
        size="sm"
        value={scenario}
        onValueChange={setScenario}
        aria-label={t('api.dns.scenario')}
        options={[
          { value: 'website', label: t('scenario.website') },
          { value: 'cdn', label: t('scenario.cdn') },
        ]}
      />
      <Input value={domain} onChange={(event) => setDomain(event.target.value)} placeholder={t('api.dns.domain')} aria-label={t('api.dns.domain')} />
      {scenario === 'website' && <Input value={ip} onChange={(event) => setIp(event.target.value)} placeholder={t('api.dns.ip')} aria-label={t('api.dns.ip')} />}
      <Input value={target} onChange={(event) => setTarget(event.target.value)} placeholder={t('api.dns.target')} aria-label={t('api.dns.target')} />
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-fg-subtle">
                <th className="py-1 font-medium">{t('dns_labels.type')}</th>
                <th className="py-1 font-medium">{t('dns_labels.name')}</th>
                <th className="py-1 font-medium">{t('dns_labels.value')}</th>
                <th className="py-1 font-medium">{t('dns_labels.purpose')}</th>
              </tr>
            </thead>
            <tbody>
              {result.records.map((record, index) => (
                <tr key={index} className="border-t border-border font-mono">
                  <td className="py-1.5">{record.type}</td>
                  <td className="py-1.5">{record.name}</td>
                  <td className="max-w-72 truncate py-1.5">{record.value}</td>
                  <td className="py-1.5 font-sans text-fg-muted">{t(`purpose.${record.purpose}`)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )
      )}
    </Panel>
  )
}

function EmailTab() {
  const { t, errorMessage } = usePlugin()
  const [domain, setDomain] = useState('acme.com') // tf-allow: 默认值
  const [mailHost, setMailHost] = useState('') // tf-allow: 默认值
  const [spf, setSpf] = useState('v=spf1 include:_spf.acme.com -all') // tf-allow: 默认值
  const [dmarc, setDmarc] = useState('v=DMARC1; p=quarantine; rua=mailto:dmarc@acme.com; pct=100') // tf-allow: 默认值

  const dnsArgs = domain.trim() ? { scenario: 'email', domain: domain.trim(), mailHost: mailHost.trim() || undefined } : null
  const dns = useDebouncedCall<DnsOut>(dnsArgs ? 'dns' : null, dnsArgs, [domain, mailHost])
  const spfArgs = spf.trim() ? { record: spf.trim() } : null
  const spfResult = useDebouncedCall<SpfOut>(spfArgs ? 'validate_spf' : null, spfArgs, [spf])
  const dmarcArgs = dmarc.trim() ? { record: dmarc.trim() } : null
  const dmarcResult = useDebouncedCall<DmarcOut>(dmarcArgs ? 'validate_dmarc' : null, dmarcArgs, [dmarc])

  return (
    <Panel icon={<Mail />} title={t('tab.email')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-3 p-3" extra={dns.pending && <Spinner className="size-3.5" />}>
      <div className="grid grid-cols-2 gap-2">
        <Input value={domain} onChange={(event) => setDomain(event.target.value)} placeholder={t('api.dns.domain')} aria-label={t('api.dns.domain')} />
        <Input value={mailHost} onChange={(event) => setMailHost(event.target.value)} placeholder={t('api.dns.mailHost')} aria-label={t('api.dns.mailHost')} />
      </div>
      {dns.result && (
        <table className="w-full text-xs">
          <tbody>
            {dns.result.records.map((record, index) => (
              <tr key={index} className="border-t border-border font-mono">
                <td className="py-1.5 pr-2">{record.type}</td>
                <td className="py-1.5 pr-2">{record.name}</td>
                <td className="py-1.5 font-sans text-fg-muted">{t(`purpose.${record.purpose}`)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <div className="h-px bg-border" />
      <Input value={spf} onChange={(event) => setSpf(event.target.value)} placeholder="SPF" aria-label="SPF" />
      {spfResult.result && (
        <div className="space-y-1">
          <Badge variant={spfResult.result.valid ? 'success' : 'danger'}>
            {spfResult.result.valid ? t('spf.valid') : t('spf.issues')}
          </Badge>
          <ValueRow label={t('spf.policy')} value={spfResult.result.policy} labelWidth="w-24" />
          {spfResult.result.issues.map((issue) => (
            <p key={issue} className="pl-[calc(6rem+0.5rem)] text-[11px] text-danger">
              {issue}
            </p>
          ))}
        </div>
      )}
      {spfResult.error && <p className="text-xs text-danger">{errorMessage(spfResult.error)}</p>}
      <div className="h-px bg-border" />
      <Input value={dmarc} onChange={(event) => setDmarc(event.target.value)} placeholder="DMARC" aria-label="DMARC" />
      {dmarcResult.result && (
        <div className="space-y-1">
          <Badge variant={dmarcResult.result.valid ? 'success' : 'danger'}>
            <ShieldCheck className="size-3" />
            {dmarcResult.result.valid ? t('dmarc.valid') : t('spf.issues')}
          </Badge>
          <ValueRow label={t('dmarc.policy')} value={dmarcResult.result.policy} labelWidth="w-24" />
          {dmarcResult.result.reports.map((report) => (
            <ValueRow key={report} label={t('dmarc.reports')} value={report} labelWidth="w-24" />
          ))}
          {dmarcResult.result.issues.map((issue) => (
            <p key={issue} className="pl-[calc(6rem+0.5rem)] text-[11px] text-danger">
              {issue}
            </p>
          ))}
        </div>
      )}
      {dmarcResult.error && <p className="text-xs text-danger">{errorMessage(dmarcResult.error)}</p>}
    </Panel>
  )
}
