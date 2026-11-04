import { useState } from 'react'
import { Badge, Button, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Users } from 'lucide-react'

interface ResolveOut {
  term: string
  selfAddress: string
  description: string
}

const STEPS = [
  { key: 'f', label: 'f' }, { key: 'm', label: 'm' }, { key: 'ob', label: 'ob' }, { key: 'os', label: 'os' },
  { key: 'yb', label: 'yb' }, { key: 'ys', label: 'ys' }, { key: 'h', label: 'h' }, { key: 'w', label: 'w' },
  { key: 's', label: 's' }, { key: 'd', label: 'd' },
]

export default function KinshipCalculator() {
  const { t, errorMessage } = usePlugin()
  const [chain, setChain] = useState<string[]>([])
  const args = chain.length > 0 ? { chain } : null
  const resolve = useDebouncedCall<ResolveOut>(args ? 'resolve' : null, args, [chain])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto">
      <Panel icon={<Users />} title={t('ui.pick')} bodyClassName="space-y-2.5 p-3">
        <div className="flex flex-wrap gap-1.5">
          {STEPS.map((step) => (
            <Button key={step.key} size="sm" variant="secondary" onClick={() => setChain((prev) => [...prev, step.key])}>
              {t(`ui.steps.${step.label}`)}
            </Button>
          ))}
        </div>
        {chain.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5">
            {chain.map((step, index) => (
              <Badge key={index} variant="primary">{t(`ui.steps.${step}`)}</Badge>
            ))}
            <Button size="sm" variant="ghost" onClick={() => setChain((prev) => prev.slice(0, -1))}>
              {t('ui.reset')}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setChain([])}>
              {t('ui.reset')}
            </Button>
          </div>
        )}
      </Panel>
      <Panel icon={<Users />} title={t('ui.term')} bodyClassName="space-y-2 p-3" extra={resolve.pending && <Spinner className="size-3.5" />}>
        {resolve.error ? (
          <p className="text-xs text-danger">{errorMessage(resolve.error)}</p>
        ) : (
          resolve.result && (
            <>
              <p className="text-3xl font-bold text-fg">{resolve.result.term}</p>
              <p className="text-sm text-fg-muted">{t('ui.self')}: {resolve.result.selfAddress}</p>
              <p className="text-xs text-fg-subtle">{t('ui.chainDesc')}: {resolve.result.description}</p>
            </>
          )
        )}
        {chain.length === 0 && <p className="text-xs text-fg-subtle">{t('ui.empty')}</p>}
      </Panel>
    </div>
  )
}
