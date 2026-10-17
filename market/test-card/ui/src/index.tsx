import { useEffect, useMemo, useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, NumberInput, Panel, SegmentedControl, Select, Spinner, Tabs, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ChevronDown, CreditCard, Download, ExternalLink, FlaskConical, RefreshCw, ShieldCheck, TriangleAlert } from 'lucide-react'
import { BRANDS, groupNumber, type Brand, type Category, type Checked, type Generated, type SandboxCase } from './types'
import { PaymentCard } from './PaymentCard'

type Tab = 'generate' | 'sandbox' | 'validate'
const CATEGORIES: Category[] = ['success', 'decline', 'authentication', 'fraud', 'dispute']

function Notice() {
  const { t } = usePlugin()
  return (
    <p className="flex shrink-0 items-center gap-2 rounded-control bg-warning-soft px-3 py-2 text-xs text-fg">
      <TriangleAlert className="size-4 shrink-0 text-warning" />
      {t('notice')}
    </p>
  )
}

function GenerateTab() {
  const { t, errorMessage } = usePlugin()
  const [brand, setBrand] = useState<Brand>('visa')
  const [count, setCount] = useState(10)
  const [seed, setSeed] = useState('')
  const [nonce, setNonce] = useState(0)
  const [view, setView] = useState<'cards' | 'table'>('cards')
  const { result, error, pending } = useDebouncedCall<Generated>('generate', { brand, count, seed }, [brand, count, seed, nonce])

  const save = async (format: 'csv' | 'json') => {
    if (!result) return
    try {
      const path = await host.dialog.saveFile(`test-cards.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (!path) return
      await host.fs.writeText(path, format === 'csv' ? result.csv : result.json)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(260px,320px)_minmax(0,1fr)] gap-3">
      <Panel icon={<FlaskConical />} title={t('generate.title')} bodyClassName="space-y-4 p-4">
        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('generate.brand')}</span>
          <Select<Brand> value={brand} onValueChange={setBrand} className="w-full" aria-label={t('generate.brand')} options={BRANDS.map((b) => ({ value: b, label: t(`brands.${b}`) }))} />
        </label>
        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('generate.count')}</span>
          <NumberInput value={count} onValueChange={setCount} min={1} max={1000} className="w-full" aria-label={t('generate.count')} />
        </label>
        <label className="block space-y-1.5">
          <span className="text-xs font-medium text-fg-muted">{t('generate.seed')}</span>
          <Input value={seed} onChange={(event) => setSeed(event.target.value)} placeholder={t('generate.seedPlaceholder')} aria-label={t('generate.seed')} />
          <span className="block text-[11px] text-fg-subtle">{t('generate.seedHint')}</span>
        </label>
        <Button variant="primary" block onClick={() => setNonce((n) => n + 1)} disabled={!!seed.trim()}>
          <RefreshCw />
          {t('generate.again')}
        </Button>
        <p className="text-[11px] leading-relaxed text-fg-subtle">{t('generate.hint')}</p>
      </Panel>
      <Panel
        icon={<CreditCard />}
        title={t('generate.result')}
        extra={pending ? <Spinner className="size-3.5 text-fg-subtle" /> : result && <Badge>{t('generate.mode')}</Badge>}
        actions={
          <>
            <SegmentedControl<'cards' | 'table'>
              size="sm"
              value={view}
              onValueChange={setView}
              aria-label={t('generate.view')}
              options={[
                { value: 'cards', label: t('generate.cards') },
                { value: 'table', label: t('generate.table') },
              ]}
            />
            <DropdownMenu
              trigger={
                <Button size="sm" disabled={!result}>
                  <Download />
                  {t('export.title')}
                  <ChevronDown />
                </Button>
              }
              items={[
                { key: 'copy-csv', label: t('export.copyCsv'), onSelect: () => void host.clipboard.writeText(result?.csv ?? '').then(() => toast.success(t('common:copied'))) },
                { key: 'save-csv', label: t('export.saveCsv'), onSelect: () => void save('csv') },
                { key: 'copy-json', label: t('export.copyJson'), onSelect: () => void host.clipboard.writeText(result?.json ?? '').then(() => toast.success(t('common:copied'))) },
                { key: 'save-json', label: t('export.saveJson'), onSelect: () => void save('json') },
              ]}
            />
          </>
        }
        bodyClassName="overflow-auto p-3"
      >
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? null : view === 'cards' ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(min(100%,280px),1fr))] gap-4">
            {result.cards.map((card, index) => (
              <PaymentCard key={`${index}-${card.number}`} card={card} />
            ))}
          </div>
        ) : (
          <table className="w-full text-xs">
            <thead className="text-left text-fg-subtle">
              <tr>
                {(['number', 'holder', 'expiry', 'cvc'] as const).map((key) => (
                  <th key={key} className="px-2 py-1.5 font-normal">{t(`fields.${key}`)}</th>
                ))}
              </tr>
            </thead>
            <tbody className="font-mono text-fg" data-selectable>
              {result.cards.map((card, index) => (
                <tr key={`${index}-${card.number}`} className="border-t border-border">
                  <td className="px-2 py-1.5">{card.number}</td>
                  <td className="px-2 py-1.5">{card.holder}</td>
                  <td className="px-2 py-1.5">{card.expiry}</td>
                  <td className="px-2 py-1.5">{card.cvc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
    </div>
  )
}

function SandboxTab() {
  const { t, call, errorMessage } = usePlugin()
  const [data, setData] = useState<{ cases: SandboxCase[]; source: string; verified: string } | null>(null)
  const [category, setCategory] = useState<Category | 'all'>('all')
  const [query, setQuery] = useState('')
  useEffect(() => {
    call<{ cases: SandboxCase[]; source: string; verified: string }>('sandbox').then(setData, (reason) => toast.error(errorMessage(reason)))
  }, [call, errorMessage])
  const cases = useMemo(
    () =>
      (data?.cases ?? []).filter(
        (c) => (category === 'all' || c.category === category) && (!query || c.number.includes(query.replace(/\D/g, '')) || t(`scenarios.${c.scenario}`).toLowerCase().includes(query.toLowerCase())),
      ),
    [data, category, query, t],
  )
  return (
    <Panel
      icon={<ShieldCheck />}
      title={t('sandbox.title')}
      actions={
        data && (
          <Button size="sm" variant="ghost" onClick={() => void host.openUrl(data.source)}>
            <ExternalLink />
            {t('sandbox.docs')}
          </Button>
        )
      }
      footer={data && <span>{t('sandbox.verified', { date: data.verified })}</span>}
      bodyClassName="flex min-h-0 flex-col gap-3 p-3"
    >
      <p className="shrink-0 text-xs text-fg-muted">{t('sandbox.hint')}</p>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Select<Category | 'all'>
          value={category}
          onValueChange={setCategory}
          className="w-40"
          aria-label={t('sandbox.category')}
          options={[{ value: 'all', label: t('sandbox.all') }, ...CATEGORIES.map((c) => ({ value: c, label: t(`categories.${c}`) }))]}
        />
        <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t('sandbox.search')} wrapperClassName="min-w-48 flex-1" aria-label={t('sandbox.search')} />
      </div>
      {cases.length === 0 ? (
        <Empty icon={<ShieldCheck />} title={t('sandbox.empty')} />
      ) : (
        <ul className="min-h-0 flex-1 space-y-1.5 overflow-auto">
          {cases.map((c) => (
            <li key={c.number} className="group flex items-center gap-3 rounded-control border border-border px-3 py-2">
              <Badge variant={c.category === 'success' ? 'success' : c.category === 'decline' ? 'danger' : 'info'}>{t(`categories.${c.category}`)}</Badge>
              <code className="w-44 shrink-0 font-mono text-[13px] text-fg" data-selectable>
                {groupNumber(c.number, c.brand)}
              </code>
              <div className="min-w-0 flex-1 text-xs">
                <p className="text-fg">{t(`scenarios.${c.scenario}`)}</p>
                <p className="text-fg-subtle">
                  {t(`platforms.${c.platform}`)} · {t(`brands.${c.brand}`)}
                  {c.code && <code className="ml-1.5 font-mono">{c.code}</code>}
                </p>
              </div>
              <CopyButton text={c.number} className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
            </li>
          ))}
        </ul>
      )}
    </Panel>
  )
}

function ValidateTab() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('4242 4242 4242 4242\n4242424242424241\n6011111111111117')
  const has = input.trim().length > 0
  const { result, error } = useDebouncedCall<Checked[]>(has ? 'validate' : null, has ? { input } : null, [input])
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(260px,0.8fr)_minmax(0,1.2fr)] gap-3">
      <Panel icon={<CreditCard />} title={t('validate.input')}>
        <CodeEditor value={input} onChange={setInput} language="text" placeholder={t('validate.placeholder')} aria-label={t('validate.input')} />
      </Panel>
      <Panel icon={<ShieldCheck />} title={t('validate.result')} bodyClassName="overflow-auto p-3">
        {error ? (
          <p className="text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result || result.length === 0 ? (
          <Empty icon={<ShieldCheck />} title={t('validate.empty')} />
        ) : (
          <ul className="space-y-1.5">
            {result.map((row) => (
              <li key={row.line} className={cn('rounded-control border px-3 py-2 text-xs', row.problems.length ? 'border-danger/40' : 'border-border')}>
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-fg-subtle tabular-nums">{t('validate.line', { line: row.line })}</span>
                  <code className="font-mono text-[13px] text-fg" data-selectable>
                    {groupNumber(row.digits, row.brand)}
                  </code>
                  <Badge variant={row.problems.length ? 'danger' : 'success'}>{t(row.problems.length ? 'validate.invalid' : 'validate.valid')}</Badge>
                  {row.brand && <Badge>{t(`brands.${row.brand}`)}</Badge>}
                </div>
                {row.problems.map((p) => (
                  <p key={p} className="mt-1 text-danger">
                    {t(`problems.${p}`, { length: row.length, allowed: row.allowedLengths.join(' / ') })}
                  </p>
                ))}
                {row.sandbox && (
                  <p className="mt-1 text-info">{t('validate.sandbox', { platform: t(`platforms.${row.sandbox.platform}`), scenario: t(`scenarios.${row.sandbox.scenario}`) })}</p>
                )}
              </li>
            ))}
          </ul>
        )}
        <p className="mt-3 text-[11px] text-fg-subtle">{t('validate.disclaimer')}</p>
      </Panel>
    </div>
  )
}

function TestCardTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('generate')
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <Tabs<Tab>
          value={tab}
          onValueChange={setTab}
          aria-label={t('name')}
          items={[
            { value: 'generate', label: t('tabs.generate'), icon: <FlaskConical /> },
            { value: 'sandbox', label: t('tabs.sandbox'), icon: <ShieldCheck /> },
            { value: 'validate', label: t('tabs.validate'), icon: <CreditCard /> },
          ]}
        />
      </div>
      <Notice />
      <div className="min-h-0 flex-1">
        {tab === 'generate' && <GenerateTab />}
        {tab === 'sandbox' && <SandboxTab />}
        {tab === 'validate' && <ValidateTab />}
      </div>
    </div>
  )
}

export default TestCardTool
