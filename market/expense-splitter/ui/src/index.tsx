import { useState } from 'react'
import { Badge, Button, Empty, Input, Panel, SegmentedControl, Select, Tooltip, toast } from '@toolforge/ui'
import { callPlugin, host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ListPlus, Receipt, Trash2, Users, Wallet, X } from 'lucide-react'
import type { Activity, Expense, Exported, Member, Report, Settlement, Split } from './types'

const emptyActivity = (): Activity => ({ name: '', members: [], expenses: [], settlements: [] })

interface Draft {
  id: string | null
  title: string
  date: string
  total: string
  payer: string
  mode: 'equal' | 'shares' | 'exact'
  participants: Set<string>
  weights: Record<string, string>
  amounts: Record<string, string>
}

const emptyDraft = (): Draft => ({
  id: null,
  title: '',
  date: '',
  total: '',
  payer: '',
  mode: 'equal',
  participants: new Set(),
  weights: {},
  amounts: {},
})

function centsText(cents: number): string {
  const sign = cents < 0 ? '-' : ''
  const abs = Math.abs(cents)
  return `${sign}${Math.floor(abs / 100)}.${String(abs % 100).padStart(2, '0')}`
}

function nameOf(activity: Activity, id: string): string {
  return activity.members.find((member) => member.id === id)?.name ?? id
}

function expenseToDraft(expense: Expense): Draft {
  const draft = emptyDraft()
  draft.id = expense.id
  draft.title = expense.title
  draft.date = expense.date ?? ''
  draft.total = expense.total
  draft.payer = expense.payer
  if (expense.split.mode === 'equal') {
    draft.mode = 'equal'
    draft.participants = new Set(expense.split.participants)
  } else if (expense.split.mode === 'shares') {
    draft.mode = 'shares'
    for (const weight of expense.split.weights) draft.weights[weight.id] = String(weight.shares)
  } else {
    draft.mode = 'exact'
    for (const amount of expense.split.amounts) draft.amounts[amount.id] = amount.cents
  }
  return draft
}

export default function ExpenseSplitter() {
  const { t, manifest, errorMessage } = usePlugin()
  const [activity, setActivity] = usePluginState<Activity>('activity', emptyActivity())
  const [report, setReport] = useState<Report | null>(null)
  const [failure, setFailure] = useState<string | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [memberName, setMemberName] = useState('')
  const [settleFrom, setSettleFrom] = useState('')
  const [settleTo, setSettleTo] = useState('')
  const [settleCents, setSettleCents] = useState('')
  const [settleDate, setSettleDate] = useState('')

  const recompute = async (next: Activity) => {
    setActivity(next)
    if (next.members.length === 0 || next.expenses.length === 0) {
      setReport(null)
      return
    }
    try {
      setReport(await callPlugin<Report>(manifest.id, 'settle', { ledger: next }))
      setFailure(null)
    } catch (reason) {
      setFailure(errorMessage(reason))
    }
  }

  const addMember = async () => {
    if (!memberName.trim()) return
    const id = (await callPlugin<{ id: string }>(manifest.id, 'new_id', {})).id
    const member: Member = { id, name: memberName.trim() }
    setMemberName('')
    await recompute({ ...activity, members: [...activity.members, member] })
  }

  const removeMember = async (id: string) => {
    await recompute({
      ...activity,
      members: activity.members.filter((member) => member.id !== id),
      expenses: activity.expenses.filter((expense) => expense.payer !== id),
      settlements: activity.settlements.filter((item) => item.from !== id && item.to !== id),
    })
  }

  const saveExpense = async () => {
    if (!draft) return
    if (!draft.title.trim() || !draft.total.trim() || !draft.payer) {
      toast.error(t('errors.esp.bad_total'))
      return
    }
    const split: Split =
      draft.mode === 'equal'
        ? { mode: 'equal', participants: [...draft.participants] }
        : draft.mode === 'shares'
          ? {
              mode: 'shares',
              weights: Object.entries(draft.weights)
                .filter(([, shares]) => shares !== '')
                .map(([id, shares]) => ({ id, shares: Number(shares) })),
            }
          : {
              mode: 'exact',
              amounts: Object.entries(draft.amounts)
                .filter(([, cents]) => cents !== '')
                .map(([id, cents]) => ({ id, cents })),
            }
    const expense: Expense = {
      id: draft.id ?? (await callPlugin<{ id: string }>(manifest.id, 'new_id', {})).id,
      title: draft.title.trim(),
      date: draft.date || null,
      total: draft.total.trim(),
      payer: draft.payer,
      split,
    }
    const expenses = draft.id
      ? activity.expenses.map((item) => (item.id === expense.id ? expense : item))
      : [...activity.expenses, expense]
    setDraft(null)
    await recompute({ ...activity, expenses })
  }

  const addSettlement = async () => {
    if (!settleFrom || !settleTo || settleFrom === settleTo || !settleCents.trim()) {
      toast.error(t('errors.esp.self_settlement'))
      return
    }
    const settlement: Settlement = {
      id: (await callPlugin<{ id: string }>(manifest.id, 'new_id', {})).id,
      from: settleFrom,
      to: settleTo,
      cents: settleCents.trim(),
      date: settleDate || null,
    }
    setSettleCents('')
    setSettleDate('')
    await recompute({ ...activity, settlements: [...activity.settlements, settlement] })
  }

  const exportReport = async (format: 'csv' | 'json' | 'text', save: boolean) => {
    if (!report) return
    try {
      const out = await callPlugin<Exported>(manifest.id, 'export', { format, report })
      if (save) {
        const path = await host.dialog.saveFile(`expense-report.${out.ext}`, [
          { name: out.ext.toUpperCase(), extensions: [out.ext] },
        ])
        if (!path) return
        await host.fs.writeText(path, out.content)
        toast.success(t('common:saved'))
      } else {
        await host.clipboard.writeText(out.content)
        toast.success(t('common:saved'))
      }
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const memberOptions = activity.members.map((member) => ({ value: member.id, label: member.name }))

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(380px,0.95fr)_minmax(0,1.05fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-y-auto pb-1">
        <Panel icon={<Users />} title={t('input.members')} bodyClassName="space-y-2 p-3">
          <div className="flex gap-2">
            <Input
              value={memberName}
              onChange={(event) => setMemberName(event.target.value)}
              placeholder={t('input.memberName')}
              size="sm"
              aria-label={t('input.memberName')}
              wrapperClassName="min-w-0 flex-1"
            />
            <Button size="sm" onClick={() => void addMember()} disabled={!memberName.trim()}>
              {t('input.addMember')}
            </Button>
          </div>
          {activity.members.map((member) => (
            <div key={member.id} className="flex items-center gap-2 rounded-control border border-border p-2">
              <span className="min-w-0 flex-1 truncate text-[13px] text-fg">{member.name}</span>
              <Tooltip content={t('input.removeMember')}>
                <Button size="icon-sm" variant="ghost" aria-label={t('input.removeMember')} onClick={() => void removeMember(member.id)}>
                  <Trash2 />
                </Button>
              </Tooltip>
            </div>
          ))}
        </Panel>

        <Panel
          icon={<Receipt />}
          title={t('input.expenses')}
          bodyClassName="space-y-2 p-3"
          actions={
            <Button size="sm" variant="ghost" onClick={() => setDraft(emptyDraft())} disabled={activity.members.length === 0}>
              {t('input.addExpense')}
            </Button>
          }
        >
          {activity.expenses.length === 0 && <p className="text-xs text-fg-subtle">{t('input.empty')}</p>}
          {activity.expenses.map((expense) => (
            <div key={expense.id} className="flex items-center gap-2 rounded-control border border-border p-2">
              <span className="min-w-0 flex-1 truncate text-xs text-fg">
                {t('input.expenseLine', { title: expense.title || t('input.title'), total: expense.total, payer: nameOf(activity, expense.payer) })}
              </span>
              <Button size="sm" variant="ghost" onClick={() => setDraft(expenseToDraft(expense))}>
                {t('input.editExpense')}
              </Button>
              <Tooltip content={t('input.removeExpense')}>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  aria-label={t('input.removeExpense')}
                  onClick={() => void recompute({ ...activity, expenses: activity.expenses.filter((item) => item.id !== expense.id) })}
                >
                  <Trash2 />
                </Button>
              </Tooltip>
            </div>
          ))}
        </Panel>

        <Panel icon={<Wallet />} title={t('input.settlements')} bodyClassName="space-y-2 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <Select value={settleFrom} onValueChange={setSettleFrom} size="sm" aria-label={t('input.from')} options={[{ value: '', label: t('input.from') }, ...memberOptions]} className="w-28" />
            <Select value={settleTo} onValueChange={setSettleTo} size="sm" aria-label={t('input.to')} options={[{ value: '', label: t('input.to') }, ...memberOptions]} className="w-28" />
            <Input value={settleCents} onChange={(event) => setSettleCents(event.target.value)} placeholder={t('input.settleAmount')} size="sm" aria-label={t('input.settleAmount')} wrapperClassName="w-24" spellCheck={false} />
            <Input value={settleDate} onChange={(event) => setSettleDate(event.target.value)} placeholder={t('input.datePlaceholder')} size="sm" aria-label={t('input.date')} wrapperClassName="w-32" spellCheck={false} />
            <Button size="sm" onClick={() => void addSettlement()} disabled={!settleFrom || !settleTo || !settleCents.trim()}>
              {t('input.addSettlement')}
            </Button>
          </div>
          {activity.settlements.length === 0 ? (
            <p className="text-xs text-fg-subtle">{t('result.transfersHint')}</p>
          ) : (
            activity.settlements.map((settlement) => (
              <div key={settlement.id} className="flex items-center gap-2 rounded-control border border-border p-2">
                <span className="min-w-0 flex-1 truncate text-xs text-fg">
                  {t('input.settlementLine', { from: nameOf(activity, settlement.from), to: nameOf(activity, settlement.to), cents: settlement.cents })}
                  {settlement.date ? <span className="text-fg-subtle"> {settlement.date}</span> : null}
                </span>
                <Tooltip content={t('input.removeSettlement')}>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    aria-label={t('input.removeSettlement')}
                    onClick={() => void recompute({ ...activity, settlements: activity.settlements.filter((item) => item.id !== settlement.id) })}
                  >
                    <Trash2 />
                  </Button>
                </Tooltip>
              </div>
            ))
          )}
        </Panel>
      </div>

      <Panel
        icon={<ListPlus />}
        title={t('result.title')}
        bodyClassName="h-full min-h-0 space-y-3 overflow-y-auto p-3"
        footer={
          report && (
            <div className="flex w-full flex-wrap items-center gap-2">
              <Button size="sm" variant="soft" onClick={() => void exportReport('csv', false)}>
                {t('export.csv')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('json', false)}>
                {t('export.json')}
              </Button>
              <Button size="sm" variant="soft" onClick={() => void exportReport('text', false)}>
                {t('export.text')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="sm" onClick={() => void exportReport('csv', true)}>
                  {t('export.saveCsv')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('json', true)}>
                  {t('export.saveJson')}
                </Button>
                <Button size="sm" onClick={() => void exportReport('text', true)}>
                  {t('export.saveText')}
                </Button>
              </span>
            </div>
          )
        }
      >
        {failure && <p className="text-xs text-danger" role="alert">{failure}</p>}
        {!report && !failure && (
          <div className="flex h-full items-center justify-center">
            <Empty icon={<ListPlus />} title={t('result.empty')} description={t('input.empty')} />
          </div>
        )}
        {report && (
          <>
            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.member')}</h3>
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-fg-subtle">
                    <th className="py-1 text-left font-normal">{t('result.member')}</th>
                    <th className="py-1 text-right font-normal">{t('result.paid')}</th>
                    <th className="py-1 text-right font-normal">{t('result.owed')}</th>
                    <th className="py-1 text-right font-normal">{t('result.settledOut')}/{t('result.settledIn')}</th>
                    <th className="py-1 text-right font-normal">{t('result.net')}</th>
                  </tr>
                </thead>
                <tbody>
                  {report.balances.map((balance) => (
                    <tr key={balance.id} className="border-t border-border">
                      <td className="py-1 text-fg">{balance.name}</td>
                      <td className="py-1 text-right font-mono text-fg-muted">{centsText(balance.paid)}</td>
                      <td className="py-1 text-right font-mono text-fg-muted">{centsText(balance.owed)}</td>
                      <td className="py-1 text-right font-mono text-fg-muted">{centsText(balance.settledOut)}/{centsText(balance.settledIn)}</td>
                      <td className={`py-1 text-right font-mono font-medium ${balance.net >= 0 ? 'text-success' : 'text-danger'}`}>
                        {centsText(balance.net)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="text-[11px] text-fg-subtle">{t('result.netHint')}</p>
            </section>

            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.transfers')}</h3>
              {report.transfers.length === 0 ? (
                <p className="text-xs text-success" role="status">{t('result.noTransfers')}</p>
              ) : (
                <>
                  <p className="text-[11px] text-fg-subtle">{t('result.transfersHint')}</p>
                  {report.transfers.map((transfer, index) => (
                    <div key={index} className="rounded-control border border-border bg-surface-2 p-2 text-xs">
                      {t('result.transferLine', { from: nameOf(activity, transfer.from), to: nameOf(activity, transfer.to), cents: centsText(transfer.cents) })}
                    </div>
                  ))}
                </>
              )}
            </section>

            <section className="space-y-1.5">
              <h3 className="text-xs font-semibold text-fg">{t('result.details')}</h3>
              {report.expenses.map((expense) => (
                <div key={expense.id} className="rounded-control border border-border bg-surface-2 p-2.5">
                  <p className="flex flex-wrap items-center gap-2 text-xs">
                    <span className="font-medium text-fg">{expense.title || t('input.title')}</span>
                    {expense.date && <span className="text-fg-subtle">{expense.date}</span>}
                    <Badge variant="neutral">{t('result.totalYuan', { cents: centsText(expense.totalCents) })}</Badge>
                    <span className="text-fg-muted">{nameOf(activity, expense.payer)}</span>
                  </p>
                  <p className="mt-1 flex flex-wrap gap-1.5 text-[11px] text-fg-muted">
                    {expense.allocations.map((allocation) => (
                      <code key={allocation.id} className="rounded-control bg-active px-1.5 py-0.5 font-mono">
                        {t('result.allocationLine', { name: nameOf(activity, allocation.id), cents: centsText(allocation.cents) })}
                      </code>
                    ))}
                  </p>
                </div>
              ))}
            </section>
          </>
        )}
      </Panel>

      {draft && (
        <Panel
          icon={<Receipt />}
          title={draft.id ? t('input.editExpense') : t('input.addExpense')}
          className="fixed inset-x-6 bottom-6 max-h-[70vh] overflow-y-auto shadow-modal"
          bodyClassName="space-y-2.5 p-3"
          footer={
            <div className="flex w-full items-center gap-2">
              <Button size="sm" variant="primary" onClick={() => void saveExpense()}>
                {t('input.save')}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setDraft(null)}>
                {t('input.cancelEdit')}
              </Button>
              <span className="ml-auto flex items-center gap-2">
                <Button size="icon-sm" variant="ghost" aria-label={t('input.cancelEdit')} onClick={() => setDraft(null)}>
                  <X />
                </Button>
              </span>
            </div>
          }
        >
          <div className="flex flex-wrap gap-2">
            <Input value={draft.title} onChange={(event) => setDraft({ ...draft, title: event.target.value })} placeholder={t('input.titlePlaceholder')} size="sm" aria-label={t('input.title')} wrapperClassName="min-w-40 flex-1" />
            <Input value={draft.date} onChange={(event) => setDraft({ ...draft, date: event.target.value })} placeholder={t('input.datePlaceholder')} size="sm" aria-label={t('input.date')} wrapperClassName="w-36" spellCheck={false} />
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Input value={draft.total} onChange={(event) => setDraft({ ...draft, total: event.target.value })} placeholder={t('input.totalPlaceholder')} size="sm" aria-label={t('input.total')} wrapperClassName="w-32" spellCheck={false} />
            <Select value={draft.payer} onValueChange={(payer) => setDraft({ ...draft, payer })} size="sm" aria-label={t('input.payer')} options={memberOptions} className="w-32" />
            <SegmentedControl<'equal' | 'shares' | 'exact'>
              value={draft.mode}
              onValueChange={(mode) => setDraft({ ...draft, mode })}
              options={[
                { value: 'equal', label: t('input.mode.equal') },
                { value: 'shares', label: t('input.mode.shares') },
                { value: 'exact', label: t('input.mode.exact') },
              ]}
            />
          </div>
          <div className="space-y-1.5">
            <span className="text-xs text-fg-muted">{t('input.participants')}</span>
            {activity.members.map((member) => (
              <div key={member.id} className="flex items-center gap-2">
                <label className="flex min-w-0 flex-1 items-center gap-2 text-xs text-fg">
                  <input
                    type="checkbox"
                    checked={draft.mode === 'equal' ? draft.participants.has(member.id) : true}
                    onChange={(event) => {
                      if (draft.mode !== 'equal') return
                      const next = new Set(draft.participants)
                      if (event.target.checked) next.add(member.id)
                      else next.delete(member.id)
                      setDraft({ ...draft, participants: next })
                    }}
                    className="accent-primary"
                  />
                  <span className="truncate">{member.name}</span>
                </label>
                {draft.mode === 'shares' && (
                  <Input
                    value={draft.weights[member.id] ?? ''}
                    onChange={(event) => setDraft({ ...draft, weights: { ...draft.weights, [member.id]: event.target.value.replace(/[^\d]/g, '') } })}
                    placeholder={t('input.shares')}
                    size="sm"
                    aria-label={t('input.shares')}
                    wrapperClassName="w-20"
                    inputMode="numeric"
                  />
                )}
                {draft.mode === 'exact' && (
                  <Input
                    value={draft.amounts[member.id] ?? ''}
                    onChange={(event) => setDraft({ ...draft, amounts: { ...draft.amounts, [member.id]: event.target.value } })}
                    placeholder={t('input.amount')}
                    size="sm"
                    aria-label={t('input.amount')}
                    wrapperClassName="w-24"
                    spellCheck={false}
                  />
                )}
              </div>
            ))}
          </div>
        </Panel>
      )}
    </div>
  )
}
