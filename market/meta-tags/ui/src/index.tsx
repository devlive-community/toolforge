import { useState } from 'react'
import { Badge, Button, Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Code2, Tags } from 'lucide-react'

interface Warning {
  field: string
  rule: string
  actual: number
}
interface GenerateOut {
  html: string
  warnings: Warning[]
}

export default function MetaTags() {
  const { t, errorMessage } = usePlugin()
  const [title, setTitle] = useState('My Page') // tf-allow: 默认值
  const [description, setDescription] = useState('') // tf-allow: 默认值
  const [url, setUrl] = useState('') // tf-allow: 默认值
  const [image, setImage] = useState('') // tf-allow: 默认值
  const args = title.trim() ? { title: title.trim(), description: description.trim() || undefined, url: url.trim() || undefined, image: image.trim() || undefined } : null
  const generate = useDebouncedCall<GenerateOut>(args ? 'generate' : null, args, [title, description, url, image])
  const [copied, setCopied] = useState(false)

  const copy = () => {
    if (!generate.result) return
    void navigator.clipboard.writeText(generate.result.html)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<Tags />} title={t('ui.gen')} bodyClassName="space-y-2.5 p-3">
        <Input value={title} onChange={(event) => setTitle(event.target.value)} placeholder={t('api.generate.title')} aria-label={t('api.generate.title')} />
        <Input value={description} onChange={(event) => setDescription(event.target.value)} placeholder={t('api.generate.description')} aria-label={t('api.generate.description')} />
        <Input value={url} onChange={(event) => setUrl(event.target.value)} placeholder={t('api.generate.url')} aria-label={t('api.generate.url')} />
        <Input value={image} onChange={(event) => setImage(event.target.value)} placeholder={t('api.generate.image')} aria-label={t('api.generate.image')} />
      </Panel>
      <Panel icon={<Code2 />} title={t('ui.head')} className="min-h-0" bodyClassName="min-h-0 overflow-auto space-y-2 p-3" extra={generate.pending && <Spinner className="size-3.5" />}>
        {generate.error ? (
          <p className="text-xs text-danger">{errorMessage(generate.error)}</p>
        ) : (
          generate.result && (
            <>
              {generate.result.warnings.map((warning) => (
                <Badge key={warning.rule} variant="warning">{t(`warnings.${warning.rule}`)}</Badge>
              ))}
              <textarea readOnly value={generate.result.html} aria-label={t('ui.head')} className="h-56 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-[11px] text-fg outline-none" />
              <Button size="sm" variant="secondary" onClick={copy}>
                {copied ? t('ui.copy') : t('ui.copy')}
              </Button>
            </>
          )
        )}
      </Panel>
    </div>
  )
}
