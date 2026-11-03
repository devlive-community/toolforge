import { useState } from 'react'
import { Button, Input, Panel, Spinner } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, ImageDown } from 'lucide-react'

interface ComposeOut {
  svg: string
  width: number
  height: number
}

export default function MemeComposer() {
  const { t, errorMessage } = usePlugin()
  const [path, setPath] = useState('')
  const [top, setTop] = useState('') // tf-allow: 默认值
  const [bottom, setBottom] = useState('') // tf-allow: 默认值
  const args = path ? { imagePath: path, topText: top, bottomText: bottom } : null
  const compose = useDebouncedCall<ComposeOut>(args ? 'compose' : null, args, [path, top, bottom])

  const pick = async () => {
    const picked = await host.dialog.openFile([{ name: 'PNG/JPEG', extensions: ['png', 'jpg', 'jpeg'] }])
    if (picked) setPath(picked)
  }

  const exportPng = () => {
    const data = compose.result
    if (!data) return
    const image = new Image()
    image.onload = () => {
      const canvas = document.createElement('canvas')
      canvas.width = data.width
      canvas.height = data.height
      const context = canvas.getContext('2d')!
      context.drawImage(image, 0, 0)
      const anchor = document.createElement('a')
      anchor.href = canvas.toDataURL('image/png')
      anchor.download = 'meme.png'
      anchor.click()
    }
    image.src = `data:image/svg+xml;utf8,${encodeURIComponent(data.svg)}`
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(280px,0.7fr)_minmax(0,1.3fr)] gap-3">
      <Panel icon={<ImageDown />} title={t('ui.pick')} bodyClassName="space-y-2.5 p-3">
        <Button onClick={() => void pick()}>{t('ui.pick_file')}</Button>
        <Input value={path} onChange={(event) => setPath(event.target.value)} placeholder={t('api.compose.imagePath')} aria-label={t('api.compose.imagePath')} />
        <Input value={top} onChange={(event) => setTop(event.target.value)} placeholder={t('ui.top')} aria-label={t('ui.top')} />
        <Input value={bottom} onChange={(event) => setBottom(event.target.value)} placeholder={t('ui.bottom')} aria-label={t('ui.bottom')} />
        {compose.error && <p className="text-xs text-danger">{errorMessage(compose.error)}</p>}
      </Panel>
      <Panel icon={<ImageDown />} title={t('ui.compose')} className="min-h-0" bodyClassName="flex flex-col items-center justify-center gap-3 overflow-auto p-3" extra={compose.pending && <Spinner className="size-3.5" />}>
        {compose.result ? (
          <>
            <img src={`data:image/svg+xml;utf8,${encodeURIComponent(compose.result.svg)}`} alt={t('ui.compose')} className="max-h-[55vh]" />
            <Button size="sm" variant="secondary" onClick={exportPng}>
              <Download className="size-3.5" />
              {t('ui.export')}
            </Button>
          </>
        ) : (
          <p className="text-xs text-fg-subtle">{t('ui.pick')}</p>
        )}
      </Panel>
    </div>
  )
}
