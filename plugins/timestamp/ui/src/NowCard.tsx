import { useEffect, useState } from 'react'
import { Button, Spinner } from '@toolforge/ui'
import { CopyButton, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Pause, Play } from 'lucide-react'
import type { Now } from './types'

const TICK_MS = 1000

export function NowCard({ onUse }: { onUse: (seconds: number) => void }) {
  const { t, call } = usePlugin()
  const [now, setNow] = useState<Now | null>(null)
  const [paused, setPaused] = useState(false)

  useEffect(() => {
    if (paused) return
    let alive = true
    const tick = () => call<Now>('now').then((value) => alive && setNow(value)).catch(() => {})
    tick()
    const timer = setInterval(tick, TICK_MS)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [call, paused])

  if (!now) {
    return (
      <div className="flex h-28 items-center justify-center rounded-card border border-border bg-surface shadow-card">
        <Spinner className="text-fg-subtle" />
      </div>
    )
  }

  return (
    <section className="grid grid-cols-[minmax(0,auto)_minmax(0,1fr)_auto] items-center gap-x-10 rounded-card border border-border bg-surface px-5 py-4 shadow-card">
      <div className="min-w-0 space-y-1">
        <p className="text-xs font-medium text-fg-muted">{t('now.unix')}</p>
        <div className="flex items-center gap-2">
          <span className="font-mono text-3xl font-semibold tracking-tight text-fg tabular-nums" data-selectable>
            {now.seconds}
          </span>
          <CopyButton text={String(now.seconds)} />
        </div>
        <div className="flex items-center gap-1 text-xs text-fg-muted">
          {t('now.millis')}
          <span className="font-mono text-fg tabular-nums" data-selectable>
            {now.milliseconds}
          </span>
          <CopyButton text={String(now.milliseconds)} size="icon-sm" />
        </div>
      </div>
      <dl className="grid min-w-0 grid-cols-[auto_1fr] items-baseline gap-x-4 gap-y-2">
        <dt className="text-xs font-medium text-fg-muted">{t('now.local')}</dt>
        <dd className="truncate font-mono text-[15px] text-fg tabular-nums" data-selectable>
          {now.local.datetime.slice(0, 19)}
          <span className="ml-2 font-sans text-xs text-fg-subtle">
            {now.local.timezone} {now.local.offset}
          </span>
        </dd>
        <dt className="text-xs font-medium text-fg-muted">{t('now.utc')}</dt>
        <dd className="truncate font-mono text-[15px] text-fg tabular-nums" data-selectable>
          {now.iso.replace(/\.\d+Z$/, 'Z')}
        </dd>
      </dl>
      <div className="flex flex-col gap-2">
        <Button onClick={() => onUse(now.seconds)}>{t('now.use')}</Button>
        <Button onClick={() => setPaused((v) => !v)}>
          {paused ? <Play /> : <Pause />}
          {t(paused ? 'now.resume' : 'now.pause')}
        </Button>
      </div>
    </section>
  )
}
