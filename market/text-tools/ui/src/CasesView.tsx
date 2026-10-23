import { Empty, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CaseSensitive } from 'lucide-react'
import type { Converted } from './types'

export function CasesView({ input }: { input: string }) {
  const { t } = usePlugin()
  const has = input.length > 0
  const { result, pending } = useDebouncedCall<Converted[]>(has ? 'cases' : null, has ? { input } : null, [input])
  if (!has) return <Empty icon={<CaseSensitive />} title={t('empty')} />
  if (!result) return <Spinner className="m-6 text-fg-subtle" />
  return (
    <div className={pending ? 'opacity-70 transition-opacity' : undefined}>
      {result.map((item) => (
        <ValueRow
          key={item.style}
          labelWidth="w-32"
          label={
            <span className="flex flex-col">
              <span className="text-fg">{t(`styles.${item.style}.name`)}</span>
              <span className="font-mono font-normal text-fg-subtle">{t(`styles.${item.style}.example`)}</span>
            </span>
          }
          value={item.output}
        />
      ))}
    </div>
  )
}
