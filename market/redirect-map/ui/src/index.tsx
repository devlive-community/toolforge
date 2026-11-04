import { useState } from 'react'
import { Badge, Button, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRightLeft, Copy } from 'lucide-react'

interface Redirect {
  from: string
  to: string
}
interface GenerateOut {
  rules: string
  redirects: Redirect[]
  cycles: string[]
  conflicts: string[]
  chainsFlattened: number
}

const SAMPLE = '/old-page:/new-page\n/blog/2020:/posts/2020' // tf-allow: 示例输入

export default function RedirectMap() {
  const { t, errorMessage } = usePlugin()
  const [mappings, setMappings] = useState(SAMPLE) // tf-allow: 默认值
  const [format, setFormat] = useState<'nginx' | 'apache' | 'cloudflare'>('nginx')
  const pairs = mappings
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [from, to] = line.split(/[:\t]/).map((part) => part.trim())
      return [from, to] as [string, string]
    })
    .filter(([from, to]) => from && to)
  const args = pairs.length > 0 ? { mappings: pairs, format } : null
  const generate = useDebouncedCall<GenerateOut>(args ? 'generate' : null, args, [mappings, format])
  const [copied, setCopied] = useState(false)

  const copy = () => {
    if (!generate.result) return
    void navigator.clipboard.writeText(generate.result.rules)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<ArrowRightLeft />} title={t('ui.mappings')} bodyClassName="space-y-2.5 p-3">
        <SegmentedControl
          size="sm"
          value={format}
          onValueChange={setFormat}
          aria-label={t('api.generate.format')}
          options={[
            { value: 'nginx', label: 'nginx' },
            { value: 'apache', label: 'apache' },
            { value: 'cloudflare', label: 'cloudflare' },
          ]}
        />
        <textarea
          value={mappings}
          onChange={(event) => setMappings(event.target.value)}
          aria-label={t('ui.mappings')}
          className="h-48 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        {generate.result && generate.result.chainsFlattened > 0 && (
          <Badge variant="neutral">{t('ui.flattened', { count: generate.result.chainsFlattened })}</Badge>
        )}
        {generate.result && generate.result.cycles.length > 0 && (
          <div className="rounded-control bg-danger-soft p-2 text-[11px] text-danger">
            <p className="font-medium">{t('ui.cycles')}</p>
            {generate.result.cycles.map((cycle) => <p key={cycle}>{cycle}</p>)}
          </div>
        )}
        {generate.result && generate.result.conflicts.length > 0 && (
          <div className="rounded-control bg-warning-soft p-2 text-[11px] text-warning">
            <p className="font-medium">{t('ui.conflicts')}</p>
            {generate.result.conflicts.map((conflict) => <p key={conflict}>{conflict}</p>)}
          </div>
        )}
      </Panel>
      <Panel
        icon={<ArrowRightLeft />}
        title={t('ui.rules')}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-2 p-3"
        extra={generate.pending && <Spinner className="size-3.5" />}
      >
        {generate.error ? (
          <p className="text-xs text-danger">{errorMessage(generate.error)}</p>
        ) : (
          generate.result && (
            <>
              <textarea readOnly value={generate.result.rules} aria-label={t('ui.rules')} className="h-64 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-[11px] text-fg outline-none" />
              <Button size="sm" variant="secondary" onClick={copy}>
                <Copy className="size-3.5" />
                {copied ? t('ui.copy') : t('ui.copy')}
              </Button>
            </>
          )
        )}
      </Panel>
    </div>
  )
}
