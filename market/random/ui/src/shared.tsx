import { useCallback, useState, type ReactNode } from 'react'
import { Badge, Input } from '@toolforge/ui'
import { usePlugin, type AppError } from '@toolforge/plugin-ui-sdk'

export interface Meta {
  seeded: boolean
  algorithm: string
}

/** 点击按钮才抽取：随机结果不应随输入自动变化 */
export function useRun<T>() {
  const { call } = usePlugin()
  const [result, setResult] = useState<T | null>(null)
  const [error, setError] = useState<AppError | null>(null)
  const [pending, setPending] = useState(false)
  const [round, setRound] = useState(0)
  const run = useCallback(
    (fn: string, args: object) => {
      setPending(true)
      call<T>(fn, args)
        .then(
          (value) => {
            setResult(value)
            setError(null)
            setRound((n) => n + 1)
          },
          (e: AppError) => setError(e),
        )
        .finally(() => setPending(false))
    },
    [call],
  )
  return { result, error, pending, round, run }
}

/** 三个页签共用一个种子：由外层持有后传入 */
export interface SeedProps {
  seed: string
  setSeed: (seed: string) => void
}

export function Field({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <div className="text-xs font-medium text-fg-muted">{label}</div>
      {children}
      {hint && <p className="text-[11px] leading-relaxed text-fg-subtle">{hint}</p>}
    </div>
  )
}

export function ToggleRow({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-[13px] text-fg">
      {label}
      {children}
    </label>
  )
}

export function SeedField({ seed, setSeed }: SeedProps) {
  const { t } = usePlugin()
  return (
    <Field label={t('seed.label')} hint={t('seed.hint')}>
      <Input value={seed} onChange={(e) => setSeed(e.target.value)} placeholder={t('seed.placeholder')} aria-label={t('seed.label')} />
    </Field>
  )
}

/** 结果标题栏上的说明：是否可复现 */
export function MetaBadge({ meta, seed }: { meta: Meta | null; seed: string }) {
  const { t } = usePlugin()
  if (!meta) return null
  return meta.seeded ? (
    <Badge variant="success" title={meta.algorithm}>
      {t('seed.verifiable', { seed: seed.trim() })}
    </Badge>
  ) : (
    <Badge>{t('seed.random')}</Badge>
  )
}
