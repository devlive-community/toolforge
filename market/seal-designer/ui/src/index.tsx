import { useState } from 'react'
import { Button, Input, Panel, SegmentedControl } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, Stamp } from 'lucide-react'

interface RenderOut {
  svg: string
  order: string[]
}

export default function SealDesigner() {
  const { t } = usePlugin()
  const [text, setText] = useState('平安喜乐') // tf-allow: 默认值
  const [shape, setShape] = useState<'square' | 'rect' | 'round'>('square')
  const [style, setStyle] = useState<'zhuwen' | 'baiwen'>('zhuwen')
  const [mirror, setMirror] = useState(false)
  const args = text.trim().length >= 2 ? { text: text.trim(), shape, style, mirror, transparent: true } : null
  const render = useDebouncedCall<RenderOut>(args ? 'render' : null, args, [text, shape, style, mirror])

  const download = () => {
    if (!render.result) return
    const blob = new Blob([render.result.svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'seal.svg'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<Stamp />} title={t('ui.text')} bodyClassName="space-y-2.5 p-3">
        <Input value={text} onChange={(event) => setText(event.target.value)} placeholder={t('ui.text')} aria-label={t('ui.text')} />
        <SegmentedControl size="sm" value={shape} onValueChange={(value) => setShape(value as typeof shape)} aria-label={t('ui.shape')}
          options={[{ value: 'square', label: t('ui.shapes.square') }, { value: 'rect', label: t('ui.shapes.rect') }, { value: 'round', label: t('ui.shapes.round') }]} />
        <SegmentedControl size="sm" value={style} onValueChange={(value) => setStyle(value as typeof style)} aria-label={t('ui.style')}
          options={[{ value: 'zhuwen', label: t('ui.styles.zhuwen') }, { value: 'baiwen', label: t('ui.styles.baiwen') }]} />
        <label className="flex items-center gap-1.5 text-xs text-fg-muted">
          <input type="checkbox" checked={mirror} onChange={(event) => setMirror(event.target.checked)} className="size-3.5" />
          {t('ui.mirror')}
        </label>
        {render.result && (
          <p className="text-xs text-fg-muted">{t('ui.text')}: {render.result.order.join(' ')}</p>
        )}
      </Panel>
      <Panel icon={<Stamp />} title={t('ui.export')} className="min-h-0" bodyClassName="flex flex-col items-center justify-center gap-3 p-3">
        {render.result ? (
          <>
            <img src={`data:image/svg+xml;utf8,${encodeURIComponent(render.result.svg)}`} alt={t('ui.text')} className="max-h-[50vh]" />
            <Button size="sm" variant="secondary" onClick={download}>
              <Download className="size-3.5" />
              {t('ui.download')}
            </Button>
          </>
        ) : (
          <p className="text-xs text-fg-subtle">{t('ui.text')}</p>
        )}
      </Panel>
    </div>
  )
}
