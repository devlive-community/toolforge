import { useState } from 'react'
import { Button, CodeEditor, Input, SegmentedControl } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { FormKind, Kind, Op, View } from './types'

const FIELDS: Record<FormKind, string[]> = {
  project: ['name', 'partName', 'stitches'], part: ['name', 'stitches'], copy: ['name'],
  segment: ['name', 'rows', 'repeats'], reminder: ['name', 'start', 'interval', 'repeats', 'delta'],
  details: ['projectName', 'partName', 'note'], adjust: ['stitches'],
}

export function Editor({ kind, view, busy, onSave, onCancel }: { kind: FormKind; view: View; busy: boolean; onSave: (op: Op, follow: boolean) => void; onCancel: () => void }) {
  const { t } = usePlugin()
  const [values, setValues] = useState<Record<string, string>>(() => ({
    name: kind === 'copy' ? t('part.copyName', { name: view.part?.name }) : '',
    partName: kind === 'details' ? view.part?.name ?? '' : t('part.defaultName'),
    projectName: view.projectName ?? '', note: view.part?.note ?? '',
    stitches: String(kind === 'adjust' ? view.part?.stitches ?? 40 : 40),
    rows: '4', repeats: '8', start: String(view.part?.nextLine ?? 1), interval: '2', delta: '2',
  }))
  const [mode, setMode] = useState<Kind>(view.part?.kind ?? 'rows')
  const submit = () => {
    const type = { project: 'newProject', part: 'newPart', copy: 'copyPart', segment: 'addSegment', reminder: 'addReminder', details: 'details', adjust: 'adjust' }[kind]
    const op: Op = { type }
    for (const field of FIELDS[kind]) op[field] = values[field]
    if (kind === 'project' || kind === 'part') op.kind = mode
    onSave(op, ['project', 'part', 'copy'].includes(kind))
  }
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div><h3 className="text-base font-semibold text-fg">{t(`forms.${kind}`)}</h3><p className="mt-1 text-xs leading-relaxed text-fg-muted">{t(`formHints.${kind}`)}</p></div>
      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {(kind === 'project' || kind === 'part') && <div className={busy ? 'pointer-events-none opacity-50' : ''}><SegmentedControl value={mode} onValueChange={(value) => { if (!busy) setMode(value) }} size="sm" aria-label={t('fields.mode')} options={[{ value: 'rows', label: t('mode.rows') }, { value: 'rounds', label: t('mode.rounds') }]} /></div>}
        {FIELDS[kind].map((field) => <label key={field} className="block space-y-1.5"><span className="text-xs font-medium text-fg">{t(`fields.${field}`)}</span>
          {field === 'note' ? <div className="h-36 rounded-control border border-border"><CodeEditor value={values[field]} onChange={(value) => setValues({ ...values, [field]: value })} readOnly={busy} language="text" aria-label={t(`fields.${field}`)} /></div> :
            <Input value={values[field]} disabled={busy} onChange={(event) => setValues({ ...values, [field]: event.target.value })} aria-label={t(`fields.${field}`)} maxLength={['name', 'partName', 'projectName'].includes(field) ? 80 : 16} inputMode={['stitches', 'rows', 'repeats', 'start', 'interval'].includes(field) ? 'numeric' : 'text'} />}
        </label>)}
      </div>
      <div className="flex shrink-0 items-center gap-2 border-t border-border pt-3"><Button variant="primary" disabled={busy} onClick={submit}>{t('common.save')}</Button><Button variant="ghost" disabled={busy} onClick={onCancel}>{t('common.cancel')}</Button></div>
    </div>
  )
}
