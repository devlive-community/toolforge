import { useState } from 'react'
import { Button, Input, Panel, SegmentedControl, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileText, Receipt, ScrollText } from 'lucide-react'

interface SplitOut {
  rate: number
  gross: number
  net: number
  tax: number
}
interface UpperOut {
  text: string
  valid: boolean
}
interface ReimbOut {
  markdown: string
  total: number
  taxTotal: number
  count: number
}

const RATES = [13, 9, 6, 3, 1, 0]
interface Item {
  name: string
  amount: number
  rate: number
  category: string
}

export default function InvoiceAssistant() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'split' | 'uppercase' | 'reimb'>('split')

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <SegmentedControl
        value={tab}
        onValueChange={setTab}
        aria-label={t('tab.label')}
        options={[
          { value: 'split', label: t('tab.split') },
          { value: 'uppercase', label: t('tab.uppercase') },
          { value: 'reimb', label: t('tab.reimb') },
        ]}
      />
      {tab === 'split' ? <SplitTab /> : tab === 'uppercase' ? <UppercaseTab /> : <ReimbTab />}
    </div>
  )
}

function SplitTab() {
  const { t, errorMessage } = usePlugin()
  const [amount, setAmount] = useState(113) // tf-allow: 默认值
  const [rate, setRate] = useState(13)
  const [included, setIncluded] = useState<'true' | 'false'>('true')
  const args = amount >= 0 ? { amount, rate, taxIncluded: included === 'true' } : null
  const { result, error, pending } = useDebouncedCall<SplitOut>(args ? 'tax_split' : null, args, [amount, rate, included])

  return (
    <Panel icon={<Receipt />} title={t('tab.split')} className="min-h-0 flex-1" bodyClassName="space-y-2.5 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <div className="grid grid-cols-[1fr_140px] gap-2">
        <Input type="number" value={String(amount)} onChange={(event) => setAmount(Number(event.target.value))} placeholder={t('api.tax_split.amount')} aria-label={t('api.tax_split.amount')} />
        <Select value={String(rate)} onValueChange={(value) => setRate(Number(value))} options={RATES.map((value) => ({ value: String(value), label: `${value}%` }))} aria-label={t('api.tax_split.rate')} />
      </div>
      <SegmentedControl
        size="sm"
        value={included}
        onValueChange={setIncluded}
        aria-label={t('api.tax_split.taxIncluded')}
        options={[
          { value: 'true', label: t('split.included') },
          { value: 'false', label: t('split.excluded') },
        ]}
      />
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <div className="space-y-1">
            <ValueRow label={t('split.gross')} value={result.gross.toFixed(2)} labelWidth="w-32" />
            <ValueRow label={t('split.tax')} value={result.tax.toFixed(2)} labelWidth="w-32" />
            <ValueRow label={t('split.net')} value={result.net.toFixed(2)} labelWidth="w-32" />
          </div>
        )
      )}
    </Panel>
  )
}

function UppercaseTab() {
  const { t, errorMessage } = usePlugin()
  const [amount, setAmount] = useState(1234.56) // tf-allow: 默认值
  const args = amount >= 0 ? { amount } : null
  const { result, error, pending } = useDebouncedCall<UpperOut>(args ? 'uppercase' : null, args, [amount])

  return (
    <Panel icon={<ScrollText />} title={t('tab.uppercase')} className="min-h-0 flex-1" bodyClassName="space-y-2.5 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <Input type="number" value={String(amount)} onChange={(event) => setAmount(Number(event.target.value))} placeholder={t('api.uppercase.amount')} aria-label={t('api.uppercase.amount')} />
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <div className="rounded-control bg-surface-2 p-3 text-lg font-medium tracking-wide text-fg">
            {result.valid ? result.text : t('errors.invoice.bad_amount', { amount })}
          </div>
        )
      )}
    </Panel>
  )
}

function ReimbTab() {
  const { t, errorMessage } = usePlugin()
  const [title, setTitle] = useState('') // tf-allow: 默认值
  const [person, setPerson] = useState('') // tf-allow: 默认值
  const [items, setItems] = useState<Item[]>([{ name: '', amount: 0, rate: 13, category: '' }]) // tf-allow: 默认值
  const args =
    title.trim() && person.trim() && items.every((item) => item.name.trim() && item.amount > 0)
      ? { title: title.trim(), person: person.trim(), items: items.map((item) => ({ ...item, name: item.name.trim(), category: item.category.trim() || undefined })) }
      : null
  const { result, error, pending } = useDebouncedCall<ReimbOut>(args ? 'reimbursement' : null, args, [title, person, items])
  const [copied, setCopied] = useState(false)

  const copy = async () => {
    if (!result) return
    await navigator.clipboard.writeText(result.markdown)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <Panel icon={<FileText />} title={t('reimb.title')} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-2.5 p-3" extra={pending && <Spinner className="size-3.5" />}>
      <div className="grid grid-cols-2 gap-2">
        <Input value={title} onChange={(event) => setTitle(event.target.value)} placeholder={t('api.reimbursement.title')} aria-label={t('api.reimbursement.title')} />
        <Input value={person} onChange={(event) => setPerson(event.target.value)} placeholder={t('api.reimbursement.person')} aria-label={t('api.reimbursement.person')} />
      </div>
      {items.map((item, index) => (
        <div key={index} className="grid grid-cols-[1.2fr_100px_90px_1fr_auto] items-center gap-2">
          <Input size="sm" value={item.name} onChange={(event) => setItems((prev) => prev.map((p, i) => (i === index ? { ...p, name: event.target.value } : p)))} placeholder={t('reimb.item')} aria-label={t('reimb.item')} />
          <Input size="sm" type="number" value={String(item.amount)} onChange={(event) => setItems((prev) => prev.map((p, i) => (i === index ? { ...p, amount: Number(event.target.value) } : p)))} placeholder={t('reimb.amount')} aria-label={t('reimb.amount')} />
          <Select size="sm" value={String(item.rate)} onValueChange={(value) => setItems((prev) => prev.map((p, i) => (i === index ? { ...p, rate: Number(value) } : p)))} options={RATES.map((value) => ({ value: String(value), label: `${value}%` }))} aria-label={t('api.tax_split.rate')} />
          <Input size="sm" value={item.category} onChange={(event) => setItems((prev) => prev.map((p, i) => (i === index ? { ...p, category: event.target.value } : p)))} placeholder={t('reimb.category')} aria-label={t('reimb.category')} />
          <Button size="sm" variant="ghost" onClick={() => setItems((prev) => prev.filter((_, i) => i !== index))} aria-label={t('notes.delete')}>
            {'×'}
          </Button>
        </div>
      ))}
      <Button size="sm" variant="secondary" onClick={() => setItems((prev) => [...prev, { name: '', amount: 0, rate: 13, category: '' }])}>
        {t('reimb.add')}
      </Button>
      {error ? (
        <p className="text-xs text-danger">{errorMessage(error)}</p>
      ) : (
        result && (
          <div className="space-y-2">
            <ValueRow label={t('reimb.title')} value={`${result.count} · ${result.total.toFixed(2)} 元（税 ${result.taxTotal.toFixed(2)}）`} labelWidth="w-24" />
            <Button size="sm" variant="secondary" onClick={() => void copy()}>
              {copied ? t('copied') : t('reimb.copy')}
            </Button>
          </div>
        )
      )}
    </Panel>
  )
}
