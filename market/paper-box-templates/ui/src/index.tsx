import { useState } from 'react'
import { Badge, Button, Input, Panel, SegmentedControl } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, Package, Printer } from 'lucide-react'

interface TemplateOut {
  fits: boolean
  sheetLong: number
  sheetShort: number
  svg: string
  steps: string[]
}

type Kind = 'box' | 'envelope'

export default function PaperBoxTemplates() {
  const { t, errorMessage } = usePlugin()
  const [kind, setKind] = useState<Kind>('box')
  const [length, setLength] = useState(60) // tf-allow: 默认值
  const [width, setWidth] = useState(40) // tf-allow: 默认值
  const [height, setHeight] = useState(30) // tf-allow: 默认值
  const [cardLength, setCardLength] = useState(120) // tf-allow: 默认值
  const [cardWidth, setCardWidth] = useState(80) // tf-allow: 默认值

  const args = kind === 'box'
    ? { length, width, height }
    : { cardLength, cardWidth }
  const template = useDebouncedCall<TemplateOut>(args ? kind : null, args, [kind, length, width, height, cardLength, cardWidth])

  const download = () => {
    if (!template.result) return
    const blob = new Blob([template.result.svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = kind === 'box' ? 'box-template.svg' : 'envelope-template.svg'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.6fr)_minmax(0,1.4fr)] gap-3">
      <Panel icon={<Package />} title={t('ui.params')} bodyClassName="space-y-2.5 p-3">
        <SegmentedControl
          size="sm"
          value={kind}
          onValueChange={(value) => setKind(value as Kind)}
          aria-label={t('tab.label')}
          options={[
            { value: 'box', label: t('tab.box') },
            { value: 'envelope', label: t('tab.envelope') },
          ]}
        />
        {kind === 'box' ? (
          <>
            <Input type="number" value={String(length)} onChange={(event) => setLength(Number(event.target.value))} placeholder={t('api.box.length')} aria-label={t('api.box.length')} />
            <Input type="number" value={String(width)} onChange={(event) => setWidth(Number(event.target.value))} placeholder={t('api.box.width')} aria-label={t('api.box.width')} />
            <Input type="number" value={String(height)} onChange={(event) => setHeight(Number(event.target.value))} placeholder={t('api.box.height')} aria-label={t('api.box.height')} />
          </>
        ) : (
          <>
            <Input type="number" value={String(cardLength)} onChange={(event) => setCardLength(Number(event.target.value))} placeholder={t('api.envelope.cardLength')} aria-label={t('api.envelope.cardLength')} />
            <Input type="number" value={String(cardWidth)} onChange={(event) => setCardWidth(Number(event.target.value))} placeholder={t('api.envelope.cardWidth')} aria-label={t('api.envelope.cardWidth')} />
          </>
        )}
        <div className="flex flex-wrap gap-1.5 text-[11px]">
          <Badge variant="neutral">{t('legend.cut')}</Badge>
          <Badge variant="neutral">{t('legend.fold')}</Badge>
          <Badge variant="neutral">{t('legend.glue')}</Badge>
        </div>
        {template.result && (
          <>
            <p className={template.result.fits ? 'text-xs text-success' : 'text-xs text-danger'}>
              {template.result.fits ? t('ui.fitsYes') : t('ui.fitsNo')} · {t('ui.sheet', { long: Math.round(template.result.sheetLong), short: Math.round(template.result.sheetShort) })}
            </p>
            <div className="flex gap-2">
              <Button size="sm" variant="secondary" onClick={download}>
                <Download className="size-3.5" />
                {t('ui.export')}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => window.print()}>
                <Printer className="size-3.5" />
                {t('ui.print')}
              </Button>
            </div>
            <ol className="space-y-1 text-xs text-fg-muted">
              {template.result.steps.map((step) => (
                <li key={step}>{t(`steps.${step}`)}</li>
              ))}
            </ol>
          </>
        )}
        {template.error && <p className="text-xs text-danger">{errorMessage(template.error)}</p>}
        <p className="text-[11px] text-fg-subtle">{t('ui.note')}</p>
      </Panel>
      <Panel icon={<Package />} title={t('ui.preview')} className="min-h-0" bodyClassName="flex items-center justify-center overflow-auto p-3">
        {template.result ? (
          <img
            src={`data:image/svg+xml;utf8,${encodeURIComponent(template.result.svg)}`}
            alt={t('ui.preview')}
            className="max-h-[60vh] w-full"
          />
        ) : (
          <p className="text-xs text-fg-subtle">{t('ui.params')}</p>
        )}
      </Panel>
    </div>
  )
}
