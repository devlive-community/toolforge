import { useState, type ReactNode } from 'react'
import { Button, Input, Modal, NumberInput, SegmentedControl, Select } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { localDate } from './today'
import type { Fields, Kind, Op, SessionView, Status, WorkView } from './types'

/** 由主界面提供：提交一次操作（保存成功返回 true，失败时抛出错误供表单显示） */
export interface Actions {
  commit: (op: Op) => Promise<boolean>
  busy: boolean
}

/** 评分 1–10 显示为 0.5–5 星 */
export function stars(rating: number | null): string {
  if (!rating) return ''
  return '★'.repeat(Math.floor(rating / 2)) + (rating % 2 === 1 ? '½' : '')
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block space-y-1 text-[12px] text-fg-muted">
      <span>{label}</span>
      {children}
      {hint && <span className="block text-[11px] text-fg-subtle">{hint}</span>}
    </label>
  )
}

/** 表单弹窗：提交失败时在弹窗内显示原因，不关闭 */
function FormModal({ title, onClose, actions, op, confirmLabel, danger, children }: { title: string; onClose: () => void; actions: Actions; op: Op | null; confirmLabel: string; danger?: boolean; children?: ReactNode }) {
  const { t, errorMessage } = usePlugin()
  const [error, setError] = useState<string | null>(null)
  const submit = async () => {
    if (!op) return
    setError(null)
    try {
      if (await actions.commit(op)) onClose()
    } catch (e) {
      setError(errorMessage(e))
    }
  }
  return (
    <Modal
      open
      onOpenChange={(open) => !open && onClose()}
      title={title}
      closeLabel={t('actions.close')}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t('actions.cancel')}
          </Button>
          <Button variant={danger ? 'danger' : 'primary'} loading={actions.busy} disabled={!op || actions.busy} onClick={() => void submit()}>
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        {children}
        {error && <p role="alert" className="text-[12px] text-danger">{error}</p>}
      </div>
    </Modal>
  )
}

const toNumber = (text: string) => (text.trim() === '' ? null : Number(text))

/** 新增或编辑作品资料 */
export function WorkForm({ work, onClose, actions }: { work: WorkView | null; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [kind, setKind] = useState<Kind>(work?.kind ?? 'book')
  const [title, setTitle] = useState(work?.title ?? '')
  const [creator, setCreator] = useState(work?.creator ?? '')
  const [year, setYear] = useState(work?.year?.toString() ?? '')
  const [genre, setGenre] = useState(work?.genre ?? '')
  const [tags, setTags] = useState(work?.tags.join(', ') ?? '')
  const [total, setTotal] = useState(work?.total?.toString() ?? '')
  const [notes, setNotes] = useState(work?.notes ?? '')
  const [status, setStatus] = useState<Status>('want')
  const yearValue = toNumber(year)
  const totalValue = toNumber(total)
  const numbersOk = (yearValue === null || Number.isInteger(yearValue)) && (totalValue === null || (Number.isInteger(totalValue) && totalValue > 0))
  const fields: Fields = {
    kind,
    title,
    creator,
    year: yearValue,
    genre,
    tags: tags.split(/[,，]/).map((s) => s.trim()).filter(Boolean),
    total: totalValue,
    notes,
  }
  const op: Op | null = !title.trim() || !numbersOk ? null : work ? { kind: 'editWork', workId: work.id, fields } : { kind: 'createWork', fields, status }
  return (
    <FormModal title={work ? t('work.editTitle') : t('work.newTitle')} onClose={onClose} actions={actions} op={op} confirmLabel={t('actions.save')}>
      <SegmentedControl<Kind> size="sm" value={kind} onValueChange={setKind} options={[{ value: 'book', label: t('kind.book') }, { value: 'film', label: t('kind.film') }]} />
      <div className="grid grid-cols-2 gap-2">
        <div className="col-span-2">
          <Field label={t('work.title')}>
            <Input size="sm" value={title} onChange={(e) => setTitle(e.target.value)} maxLength={120} autoFocus />
          </Field>
        </div>
        <Field label={kind === 'book' ? t('work.author') : t('work.director')}>
          <Input size="sm" value={creator} onChange={(e) => setCreator(e.target.value)} maxLength={80} />
        </Field>
        <Field label={t('work.year')}>
          <Input size="sm" value={year} onChange={(e) => setYear(e.target.value)} inputMode="numeric" maxLength={5} />
        </Field>
        <Field label={t('work.genre')}>
          <Input size="sm" value={genre} onChange={(e) => setGenre(e.target.value)} maxLength={80} />
        </Field>
        <Field label={kind === 'book' ? t('work.totalPages') : t('work.totalMinutes')} hint={t('work.totalHint')}>
          <Input size="sm" value={total} onChange={(e) => setTotal(e.target.value)} inputMode="numeric" maxLength={6} />
        </Field>
        <div className="col-span-2">
          <Field label={t('work.tags')} hint={t('work.tagsHint')}>
            <Input size="sm" value={tags} onChange={(e) => setTags(e.target.value)} />
          </Field>
        </div>
        <div className="col-span-2">
          <Field label={t('work.notes')}>
            <Input size="sm" value={notes} onChange={(e) => setNotes(e.target.value)} maxLength={2000} />
          </Field>
        </div>
        {!work && (
          <div className="col-span-2">
            <Field label={t('work.status')}>
              <SegmentedControl<Status> size="sm" value={status} onValueChange={setStatus} options={(['want', 'doing', 'done', 'paused'] as const).map((s) => ({ value: s, label: t(`status.${s}.${kind}`) }))} />
            </Field>
          </div>
        )}
      </div>
      {!numbersOk && <p className="text-[12px] text-danger">{t('work.numbersInvalid')}</p>}
    </FormModal>
  )
}

/** 编辑一次经历：进度、日期、评分与短评（不改变完成状态） */
export function SessionForm({ session, onClose, actions }: { session: SessionView; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [start, setStart] = useState(session.start ?? '')
  const [finish, setFinish] = useState(session.finish ?? '')
  const [progress, setProgress] = useState(session.progress)
  const [rating, setRating] = useState<number>(session.rating ?? 0)
  const [review, setReview] = useState(session.review)
  const unit = session.kind === 'book' ? t('unit.pages') : t('unit.minutes')
  return (
    <FormModal
      title={t('session.editTitle', { title: session.title })}
      onClose={onClose}
      actions={actions}
      op={{ kind: 'update', sessionId: session.id, start: start.trim() || null, finish: session.completed ? finish.trim() || null : null, progress, rating: rating || null, review }}
      confirmLabel={t('actions.save')}
    >
      <div className="grid grid-cols-2 gap-2">
        <Field label={t('session.start')}>
          <Input size="sm" value={start} onChange={(e) => setStart(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
        </Field>
        {session.completed ? (
          <Field label={t('session.finish')} hint={t('session.finishHint')}>
            <Input size="sm" value={finish} onChange={(e) => setFinish(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
          </Field>
        ) : (
          <div />
        )}
        <Field label={t('session.progress', { unit })} hint={session.total ? t('session.progressOf', { total: session.total, unit }) : t('session.progressUnknown')}>
          <NumberInput size="sm" value={progress} min={0} max={session.total ?? 100000} onValueChange={setProgress} />
        </Field>
        <Field label={t('session.rating')}>
          <Select<number> size="sm" value={rating} onValueChange={setRating} options={[{ value: 0, label: t('session.noRating') }, ...Array.from({ length: 10 }, (_, i) => ({ value: i + 1, label: stars(i + 1) }))]} />
        </Field>
        <div className="col-span-2">
          <Field label={t('session.review')} hint={t('session.reviewHint')}>
            <textarea
              value={review}
              onChange={(e) => setReview(e.target.value)}
              maxLength={2000}
              rows={4}
              className="w-full resize-none rounded-control border border-border bg-surface px-3 py-2 text-[13px] text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </Field>
        </div>
      </div>
      {!session.completed && <p className="text-[12px] text-fg-muted">{t('session.notCompletedHint')}</p>}
    </FormModal>
  )
}

/** 明确确认完成；完成日可留空，之后在「待补」里补上 */
export function CompleteForm({ session, onClose, actions }: { session: SessionView; onClose: () => void; actions: Actions }) {
  const { t } = usePlugin()
  const [finish, setFinish] = useState(localDate())
  return (
    <FormModal title={t('complete.title', { title: session.title })} onClose={onClose} actions={actions} op={{ kind: 'complete', sessionId: session.id, finish: finish.trim() || null }} confirmLabel={t('complete.confirm')}>
      <Field label={t('session.finish')} hint={t('complete.hint')}>
        <Input size="sm" value={finish} onChange={(e) => setFinish(e.target.value)} placeholder="YYYY-MM-DD" maxLength={10} />
      </Field>
    </FormModal>
  )
}

/** 简单确认（删除、撤销完成、撤销一步） */
export function ConfirmForm({ title, message, op, onClose, actions, confirmLabel, danger }: { title: string; message: string; op: Op; onClose: () => void; actions: Actions; confirmLabel: string; danger?: boolean }) {
  return (
    <FormModal title={title} onClose={onClose} actions={actions} op={op} confirmLabel={confirmLabel} danger={danger}>
      <p className="text-[13px] text-fg">{message}</p>
    </FormModal>
  )
}
