import { useState } from 'react'
import { Button, CodeEditor, Input } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { Editor, Op, View } from './types'
export function Form({
  editor,
  view,
  busy,
  previewDue,
  clearPreview,
  submit,
  cancel,
}: {
  editor: Editor
  view: View
  busy: boolean
  previewDue: string | null
  clearPreview: () => void
  submit: (op: Op, preview?: boolean) => void
  cancel: () => void
}) {
  const { t } = usePlugin()
  const care = editor.care
  const [values, setValues] = useState<Record<string, string>>({
    name: editor.kind === 'editPlant' ? (view.selected?.name ?? '') : editor.kind === 'editPlan' ? (care?.name ?? '') : '',
    variety: editor.kind === 'editPlant' ? (view.selected?.variety ?? '') : '',
    location: editor.kind === 'editPlant' ? (view.selected?.location ?? '') : '',
    note: editor.kind === 'editPlant' ? (view.selected?.note ?? '') : '',
    date: view.today,
    first: care?.first ?? view.today,
    interval: String(care?.interval ?? 7),
  })
  const update = (key: string, value: string) => {
    setValues({ ...values, [key]: value })
    clearPreview()
  }
  const plant = care?.plantId ?? editor.plantId ?? view.selected?.id
  const save = (preview = false) => {
    const { name, variety, location, note, date, first, interval } = values
    let op: Op
    if (editor.kind === 'plant' || editor.kind === 'editPlant')
      op = { type: editor.kind === 'plant' ? 'newPlant' : 'editPlant', plant, name, variety, location, note }
    else if (editor.kind === 'plan' || editor.kind === 'editPlan')
      op = { type: editor.kind === 'plan' ? 'newPlan' : 'editPlan', plant, plan: care?.id, name, first, interval }
    else if (editor.kind === 'complete') op = { type: 'complete', plant, plan: care?.id, date, note }
    else if (editor.kind === 'postpone') op = { type: 'postpone', plant, plan: care?.id, date }
    else op = { type: 'addPhoto', plant, date, note, path: editor.path }
    if (editor.kind === 'plant') delete op.plant
    if (editor.kind === 'plan') delete op.plan
    submit(op, preview)
  }
  const fields =
    editor.kind === 'plant' || editor.kind === 'editPlant'
      ? ['name', 'variety', 'location', 'note']
      : editor.kind === 'plan' || editor.kind === 'editPlan'
        ? ['name', 'first', 'interval']
        : editor.kind === 'postpone'
          ? ['date']
          : ['date', 'note']
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      <div>
        <h2 className="text-base font-semibold text-fg">{t(`form.${editor.kind}`)}</h2>
        <p className="mt-1 text-xs leading-relaxed text-fg-muted">{t(`hint.${editor.kind}`)}</p>
        {care && (
          <p className="mt-2 text-sm font-medium text-primary">
            {care.plantName} · {care.name}
          </p>
        )}
      </div>
      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {fields.map((key) => (
          <label key={key} className="block space-y-1.5">
            <span className="text-xs font-medium text-fg">{t(`fields.${key}`)}</span>
            {key === 'note' ? (
              <div className="h-36 overflow-hidden rounded-control border border-border">
                <CodeEditor value={values[key]} onChange={(value) => update(key, value)} readOnly={busy} language="text" aria-label={t(`fields.${key}`)} />
              </div>
            ) : (
              <Input
                value={values[key]}
                disabled={busy}
                onChange={(e) => update(key, e.target.value)}
                aria-label={t(`fields.${key}`)}
                type={key === 'date' || key === 'first' ? 'date' : 'text'}
                inputMode={key === 'interval' ? 'numeric' : 'text'}
                maxLength={key === 'variety' ? 160 : key === 'interval' ? 4 : key === 'name' || key === 'location' ? 80 : 10}
              />
            )}
          </label>
        ))}
        {editor.kind === 'photo' && (
          <p className="rounded-control border border-warning/20 bg-warning/5 p-3 text-xs leading-relaxed text-fg-muted">{t('photos.policy')}</p>
        )}
        {editor.kind === 'editPlan' && previewDue && (
          <div role="status" className="rounded-card border border-primary/30 bg-primary-soft p-3">
            <p className="text-xs text-fg-muted">{t('form.previewResult')}</p>
            <p className="mt-1 text-lg font-semibold tabular-nums text-fg">
              {care?.due}
              <span className="mx-2 text-fg-subtle">{t('common.arrow')}</span>
              {previewDue}
            </p>
          </div>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap gap-2 border-t border-border pt-3">
        {editor.kind === 'editPlan' && (
          <Button variant="outline" disabled={busy} onClick={() => save(true)}>
            {t('form.preview')}
          </Button>
        )}
        <Button variant="primary" disabled={busy || (editor.kind === 'editPlan' && !previewDue)} onClick={() => save()}>
          {t(editor.kind === 'editPlan' ? 'form.confirmSave' : 'common.save')}
        </Button>
        <Button variant="ghost" disabled={busy} onClick={cancel}>
          {t('common.cancel')}
        </Button>
      </div>
    </div>
  )
}
