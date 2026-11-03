import { useState } from 'react'
import { Badge, Input, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Box, FileJson } from 'lucide-react'

interface Step {
  kind: string
  key: string | null
  outcome: string
  target: string | null
}

interface ResolveOut {
  subpath: string
  conditions: string[]
  status: string
  target: string | null
  listed: boolean | null
  steps: Step[]
  nodeVersion: string
}

const SAMPLE = JSON.stringify(
  {
    name: 'demo',
    exports: {
      '.': {
        types: './types.d.ts',
        import: './dist/esm/index.mjs',
        require: './dist/cjs/index.cjs',
        default: './dist/index.js',
      },
      './features/*': './src/features/*.js',
    },
  },
  null,
  2,
) // tf-allow: 示例输入

export default function PackageEntryResolver() {
  const { t, errorMessage } = usePlugin()
  const [json, setJson] = useState(SAMPLE) // tf-allow: 默认值
  const [subpath, setSubpath] = useState('.') // tf-allow: 默认值
  const [conditions, setConditions] = useState('import,node') // tf-allow: 默认值
  const [listing, setListing] = useState('') // tf-allow: 默认值

  const conditionList = conditions.split(',').map((c) => c.trim()).filter(Boolean)
  const listingList = listing.split(/[,\n]/).map((c) => c.trim()).filter(Boolean)
  const args = json.trim() && subpath.trim() ? { packageJson: json, subpath: subpath.trim(), conditions: conditionList, listing: listingList } : null
  const { result, error, pending } = useDebouncedCall<ResolveOut>(args ? 'resolve' : null, args, [json, subpath, conditions, listing])

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<FileJson />} title="package.json" className="min-h-0 overflow-hidden" bodyClassName="h-full space-y-2 p-3">
        <Input value={subpath} onChange={(event) => setSubpath(event.target.value)} placeholder={t('api.resolve.subpath')} aria-label={t('api.resolve.subpath')} />
        <Input value={conditions} onChange={(event) => setConditions(event.target.value)} placeholder={t('api.resolve.conditions')} aria-label={t('api.resolve.conditions')} />
        <Input value={listing} onChange={(event) => setListing(event.target.value)} placeholder={t('api.resolve.listing')} aria-label={t('api.resolve.listing')} />
        <div className="min-h-0 flex-1">
          <textarea
            value={json}
            onChange={(event) => setJson(event.target.value)}
            spellCheck={false}
            aria-label={t('api.resolve.packageJson')}
            className="h-full w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
        </div>
      </Panel>
      <Panel
        icon={<Box />}
        title={t('results.title')}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-2 p-3"
        extra={pending && <Spinner className="size-3.5" />}
      >
        {error ? (
          <p className="text-xs text-danger">{errorMessage(error)}</p>
        ) : (
          result && (
            <>
              <div className="flex items-center gap-2">
                <Badge variant={result.status === 'resolved' ? 'success' : result.status === 'not_in_listing' ? 'neutral' : 'danger'}>
                  {t(`status.${result.status}`)}
                </Badge>
                {result.target && <span className="truncate font-mono text-xs text-fg">{result.target}</span>}
              </div>
              <p className="text-[11px] text-fg-subtle">
                {t('result.conditions')}: {result.conditions.join(', ') || '—'} · {t('result.node')}: {result.nodeVersion}
              </p>
              {result.listed !== null && (
                <p className={`text-xs ${result.listed ? 'text-success' : 'text-warning'}`}>
                  {result.listed ? t('result.listedYes') : t('result.listedNo')}
                </p>
              )}
              {result.listed === null && result.status === 'resolved' && <p className="text-xs text-fg-subtle">{t('result.noListing')}</p>}
              <div className="space-y-1 border-t border-border pt-2">
                {result.steps.map((step, index) => (
                  <div key={index} className="flex items-baseline gap-2 text-xs">
                    <span className="w-14 shrink-0 text-fg-subtle">{t(`step.${step.kind}`)}</span>
                    {step.key && <span className="font-mono text-fg-muted">{step.key}</span>}
                    <span className={`min-w-0 flex-1 truncate ${step.outcome.includes('skip') ? 'text-fg-subtle' : 'text-fg'}`}>
                      {t(`outcomes.${step.outcome}`)}
                    </span>
                    {step.target && <span className="max-w-40 shrink-0 truncate font-mono text-[11px] text-fg-subtle">{step.target}</span>}
                  </div>
                ))}
              </div>
            </>
          )
        )}
      </Panel>
    </div>
  )
}
