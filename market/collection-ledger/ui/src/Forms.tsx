import { useEffect, useState, type ReactNode } from 'react'
import { Button, Input, Modal, NumberInput, cn } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { localDate } from './today'
import type { Fields, Impact, ItemView, LoanView, Op } from './types'

/** 由主界面提供：预览一次操作的影响、提交一次操作（保存成功返回 true） */
export interface Actions {
  preview: (op: Op) => Promise<Impact>
  commit: (op: Op) => Promise<boolean>
  busy: boolean
}

const PREVIEW_DELAY = 250

/** 影响的一行：前 → 后，有变化时高亮 */
function Change({ label, value }: { label: string; value: [number, number] }) {
  const changed = value[0] !== value[1]
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-fg-muted">{label}</span>
      <span className={cn('tabular-nums', changed ? 'font-semibold text-fg' : 'text-fg-muted')}>
        {`${value[0]} → ${value[1]}`}
      </span>
    </div>
  )
}

export function ImpactView({ impact }: { impact: Impact }) {
  const { t } = usePlugin()
  return (
    <div className="space-y-1 rounded-card border border-border bg-bg p-3 text-[12px]">
      <p className="font-medium text-fg">{t('impact.title', { name: impact.itemName })}</p>
      <Change label={t('counts.owned')} value={impact.owned} />
      <Change label={t('counts.available')} value={impact.available} />
      <Change label={t('counts.outstanding')} value={impact.outstanding} />
      {impact.loanOutstanding && <Change label={t('impact.loan', { borrower: impact.borrower ?? '' })} value={impact.loanOutstanding} />}
    </div>
  )
}

/**
 * 操作弹窗：表单变化后自动预览（Rust 校验 + 影响数量），确认后提交。
 * 预览失败（如超出在手量）直接显示原因，确认按钮不可用。
 */
function ActionModal({
  open,
  onClose,
  title,
  op,
  actions,
  confirmLabel,
  danger,
  children,
}: {
  open: boolean
  onClose: () => void
  title: string
  /** 当前表单对应的操作；表单不完整时为 null */
  op: Op | null
  actions: Actions
  confirmLabel: string
  danger?: boolean
  children?: ReactNode
}) {
  const { t, errorMessage } = usePlugin()
  const [result, setResult] = useState<{ key: string; impact: Impact | null; error: string | null } | null>(null)
  const key = op ? JSON.stringify(op) : ''
  const current = result && result.key === key ? result : null

  useEffect(() => {
    if (!open || !op) return
    let alive = true
    const timer = window.setTimeout(() => {
      actions
        .preview(op)
        .then((impact) => alive && setResult({ key, impact, error: null }))
        .catch((error) => alive && setResult({ key, impact: null, error: errorMessage(error) }))
    }, PREVIEW_DELAY)
    return () => {
      alive = false
      window.clearTimeout(timer)
    }
    // key 已经覆盖 op 的内容
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, key])

  const confirm = async () => {
    if (!op || !current?.impact) return
    if (await actions.commit(op)) onClose()
  }

  return (
    <Modal
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={title}
      closeLabel={t('actions.close')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('actions.cancel')}
          </Button>
          <Button variant={danger ? 'danger' : 'primary'} loading={actions.busy} disabled={!current?.impact || actions.busy} onClick={() => void confirm()}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        {children}
        {op && !current && <p className="text-[12px] text-fg-muted">{t('impact.checking')}</p>}
        {current?.error && <p className="text-[12px] text-danger">{current.error}</p>}
        {current?.impact && <ImpactView impact={current.impact} />}
      </div>
    </Modal>
  )
}

function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="block space-y-1 text-[12px] text-fg-muted">
      <span>{label}</span>
      {children}
      {hint && <span className="block text-[11px] text-fg-subtle">{hint}</span>}
    </label>
  )
}

const emptyFields = (): Fields => ({ name: '', series: '', edition: '', code: '', category: '', location: '', condition: '', notes: '' })

/** 新增或编辑藏品资料（保有量只在新增时填写，之后用「调整数量」） */
export function ItemForm({ item, onClose, actions }: { item: ItemView | null; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [fields, setFields] = useState<Fields>(() => (item ? { ...item } : emptyFields()))
  const [owned, setOwned] = useState(1)
  const set = (key: keyof Fields) => (event: React.ChangeEvent<HTMLInputElement>) => setFields((f) => ({ ...f, [key]: event.target.value }))
  const valid = fields.name.trim().length > 0
  const op: Op | null = !valid ? null : item ? { kind: 'edit', itemId: item.id, fields } : { kind: 'create', fields, owned }
  return (
    <ActionModal open onClose={onClose} title={item ? t('item.editTitle') : t('item.newTitle')} op={op} actions={actions} confirmLabel={t('actions.save')}>
      <div className="grid grid-cols-2 gap-2">
        <div className="col-span-2">
          <Field label={t('item.name')}>
            <Input size="sm" value={fields.name} onChange={set('name')} maxLength={80} autoFocus />
          </Field>
        </div>
        <Field label={t('item.series')}>
          <Input size="sm" value={fields.series} onChange={set('series')} maxLength={80} />
        </Field>
        <Field label={t('item.edition')}>
          <Input size="sm" value={fields.edition} onChange={set('edition')} maxLength={80} />
        </Field>
        <Field label={t('item.code')}>
          <Input size="sm" value={fields.code} onChange={set('code')} maxLength={80} />
        </Field>
        <Field label={t('item.category')}>
          <Input size="sm" value={fields.category} onChange={set('category')} maxLength={80} />
        </Field>
        <Field label={t('item.location')}>
          <Input size="sm" value={fields.location} onChange={set('location')} maxLength={80} />
        </Field>
        <Field label={t('item.condition')}>
          <Input size="sm" value={fields.condition} onChange={set('condition')} maxLength={80} />
        </Field>
        {!item && (
          <Field label={t('counts.owned')}>
            <NumberInput size="sm" value={owned} min={1} max={100000} onValueChange={setOwned} />
          </Field>
        )}
        <div className="col-span-2">
          <Field label={t('item.notes')}>
            <Input size="sm" value={fields.notes} onChange={set('notes')} maxLength={500} />
          </Field>
        </div>
      </div>
    </ActionModal>
  )
}

/** 调整保有量：不能低于尚未归还的数量 */
export function OwnedForm({ item, onClose, actions }: { item: ItemView; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [owned, setOwned] = useState(item.owned)
  return (
    <ActionModal open onClose={onClose} title={t('owned.title', { name: item.name })} op={{ kind: 'setOwned', itemId: item.id, owned }} actions={actions} confirmLabel={t('actions.save')}>
      <Field label={t('counts.owned')} hint={t('owned.hint', { outstanding: item.outstanding })}>
        <NumberInput size="sm" value={owned} min={0} max={100000} onValueChange={setOwned} />
      </Field>
    </ActionModal>
  )
}

/** 借出：选择份数、借用人昵称、日期与可选预计归还日 */
export function LendForm({ item, onClose, actions }: { item: ItemView; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [borrower, setBorrower] = useState('')
  const [qty, setQty] = useState(1)
  const [date, setDate] = useState(localDate())
  const [due, setDue] = useState('')
  const [note, setNote] = useState('')
  const op: Op | null = borrower.trim() ? { kind: 'lend', itemId: item.id, borrower, qty, date, due: due.trim() || null, note } : null
  return (
    <ActionModal open onClose={onClose} title={t('lend.title', { name: item.name })} op={op} actions={actions} confirmLabel={t('lend.confirm')}>
      <div className="grid grid-cols-2 gap-2">
        <div className="col-span-2">
          <Field label={t('lend.borrower')} hint={t('lend.borrowerHint')}>
            <Input size="sm" value={borrower} onChange={(e) => setBorrower(e.target.value)} maxLength={80} autoFocus />
          </Field>
        </div>
        <Field label={t('lend.qty')} hint={t('lend.qtyHint', { available: item.available })}>
          <NumberInput size="sm" value={qty} min={1} max={Math.max(1, item.available)} onValueChange={setQty} />
        </Field>
        <Field label={t('lend.date')}>
          <Input size="sm" value={date} onChange={(e) => setDate(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
        </Field>
        <Field label={t('lend.due')} hint={t('lend.dueHint')}>
          <Input size="sm" value={due} onChange={(e) => setDue(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
        </Field>
        <Field label={t('lend.note')}>
          <Input size="sm" value={note} onChange={(e) => setNote(e.target.value)} maxLength={500} />
        </Field>
      </div>
    </ActionModal>
  )
}

/** 从具体借出单登记全部或部分归还 */
export function ReturnForm({ loan, onClose, actions }: { loan: LoanView; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [qty, setQty] = useState(loan.outstanding)
  const [date, setDate] = useState(localDate())
  const [note, setNote] = useState('')
  return (
    <ActionModal
      open
      onClose={onClose}
      title={t('return.title', { name: loan.itemName, borrower: loan.borrower })}
      op={{ kind: 'return', loanId: loan.id, qty, date, note }}
      actions={actions}
      confirmLabel={t('return.confirm')}
    >
      <p className="text-[12px] text-fg-muted">{t('return.summary', { qty: loan.qty, returned: loan.returned, outstanding: loan.outstanding, date: loan.date })}</p>
      <div className="grid grid-cols-2 gap-2">
        <Field label={t('return.qty')}>
          <NumberInput size="sm" value={qty} min={1} max={Math.max(1, loan.outstanding)} onValueChange={setQty} />
        </Field>
        <Field label={t('return.date')}>
          <Input size="sm" value={date} onChange={(e) => setDate(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
        </Field>
        <div className="col-span-2">
          <Field label={t('return.note')}>
            <Input size="sm" value={note} onChange={(e) => setNote(e.target.value)} maxLength={500} />
          </Field>
        </div>
      </div>
    </ActionModal>
  )
}

/** 简单确认（归档、删除、撤销）：同样先展示影响 */
export function ConfirmForm({ title, message, op, onClose, actions, confirmLabel, danger }: { title: string; message: string; op: Op; onClose: () => void; actions: Actions; confirmLabel: string; danger?: boolean }) {
  return (
    <ActionModal open onClose={onClose} title={title} op={op} actions={actions} confirmLabel={confirmLabel} danger={danger}>
      <p className="text-[13px] text-fg">{message}</p>
    </ActionModal>
  )
}
