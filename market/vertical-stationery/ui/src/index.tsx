import { useState } from 'react'
import { Button, Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, ScrollText } from 'lucide-react'

interface LayoutOut {
  svg: string
  columns: number
  pages: number
}

export default function VerticalStationery() {
  const { t, errorMessage } = usePlugin()
  const [text, setText] = useState('空山新雨后天气晚来秋明月松间照清泉石上流') // tf-allow: 默认值
  const [title, setTitle] = useState('山居秋暝') // tf-allow: 默认值
  const [signature, setSignature] = useState('王维') // tf-allow: 默认值
  const [fontMm, setFontMm] = useState(7) // tf-allow: 默认值
  const args = text.trim() ? { text, title: title.trim() || undefined, signature: signature.trim() || undefined, fontMm } : null
  const layout = useDebouncedCall<LayoutOut>(args ? 'layout' : null, args, [text, title, signature, fontMm])

  const download = () => {
    if (!layout.result) return
    const blob = new Blob([layout.result.svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'stationery.svg'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<ScrollText />} title={t('ui.text')} bodyClassName="space-y-2.5 p-3">
        <Input value={text} onChange={(event) => setText(event.target.value)} placeholder={t('ui.text')} aria-label={t('ui.text')} />
        <Input value={title} onChange={(event) => setTitle(event.target.value)} placeholder={t('ui.title')} aria-label={t('ui.title')} />
        <Input value={signature} onChange={(event) => setSignature(event.target.value)} placeholder={t('ui.signature')} aria-label={t('ui.signature')} />
        <label className="flex items-center gap-2 text-xs text-fg-muted">
          {t('ui.font')}
          <input type="range" min={3} max={20} value={fontMm} onChange={(event) => setFontMm(Number(event.target.value))} className="min-w-0 flex-1 accent-primary" aria-label={t('ui.font')} />
          <span className="w-8 text-right tabular-nums">{fontMm}</span>
        </label>
        {layout.result && <p className="text-xs text-fg-muted">{t('ui.pages', { count: layout.result.pages })}</p>}
        {layout.error && <p className="text-xs text-danger">{errorMessage(layout.error)}</p>}
        <Button size="sm" variant="secondary" onClick={download} disabled={!layout.result}>
          <Download className="size-3.5" />
          {t('ui.export')}
        </Button>
      </Panel>
      <Panel icon={<ScrollText />} title={t('ui.pages', { count: layout.result?.pages ?? 0 })} className="min-h-0" bodyClassName="flex items-center justify-center overflow-auto p-3" extra={layout.pending && <Spinner className="size-3.5" />}>
        {layout.result ? (
          <img src={`data:image/svg+xml;utf8,${encodeURIComponent(layout.result.svg)}`} alt={t('ui.text')} className="max-h-full" />
        ) : (
          <p className="text-xs text-fg-subtle">{t('ui.text')}</p>
        )}
      </Panel>
    </div>
  )
}
