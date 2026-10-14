import { useCallback, useEffect, useState } from 'react'
import { Badge, Button, Empty, SegmentedControl, Spinner, Tabs, toast } from '@toolforge/ui'
import { usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { AppWindow, Gauge, Keyboard as KeyboardIcon, Mouse as MouseIcon, Settings2, ShieldAlert, TriangleAlert } from 'lucide-react'
import { Apps } from './Apps'
import { Keyboard } from './Keyboard'
import { Mouse } from './Mouse'
import { Overview } from './Overview'
import { Settings } from './Settings'
import type { Range, Status, Summary } from './types'

type Tab = 'overview' | 'keyboard' | 'mouse' | 'apps' | 'settings'

const RANGES: Range[] = ['today', 'yesterday', 'week', 'month', 'all']
/** 状态（含今日计数）的刷新间隔 */
const STATUS_INTERVAL = 3000
/** 正在统计时，包含今天的范围自动刷新 */
const SUMMARY_INTERVAL = 5000

function StatusBadge({ status }: { status: Status }) {
  const { t } = usePlugin()
  if (status.capturing) {
    return (
      <Badge variant="success" className="max-w-64 gap-1.5">
        <span className="size-1.5 shrink-0 animate-pulse rounded-full bg-success" />
        <span className="truncate">{status.app ? t('status.capturingApp', { app: status.app.name }) : t('status.capturing')}</span>
      </Badge>
    )
  }
  if (status.enabled) return <Badge variant="warning">{status.permission === 'denied' ? t('status.permission') : t('status.starting')}</Badge>
  return <Badge>{t('status.paused')}</Badge>
}

function InputStats() {
  const { t, call, errorMessage } = usePlugin()
  const [tab, setTab] = usePluginState<Tab>('tab', 'overview')
  const [range, setRange] = usePluginState<Range>('range', 'today')
  const [status, setStatus] = useState<Status | null>(null)
  const [summary, setSummary] = useState<{ range: Range; data: Summary } | null>(null)
  const [version, setVersion] = useState(0)
  const [error, setError] = useState<string | null>(null)

  const fail = useCallback((reason: unknown) => toast.error(errorMessage(reason)), [errorMessage])

  useEffect(() => {
    let alive = true
    const tick = () =>
      call<Status>('status').then(
        (next) => {
          if (!alive) return
          setStatus(next)
          setError(null)
        },
        (reason) => alive && setError(errorMessage(reason)),
      )
    tick()
    const timer = setInterval(tick, STATUS_INTERVAL)
    return () => {
      alive = false
      clearInterval(timer)
    }
  }, [call, errorMessage])

  useEffect(() => {
    let alive = true
    call<Summary>('summary', { range }).then(
      (data) => alive && setSummary({ range, data }),
      (reason) => alive && setError(errorMessage(reason)),
    )
    return () => {
      alive = false
    }
  }, [call, errorMessage, range, version])

  // 统计进行中且范围包含今天时定时刷新
  const live = !!status?.capturing && range !== 'yesterday'
  useEffect(() => {
    if (!live) return
    const timer = setInterval(() => setVersion((v) => v + 1), SUMMARY_INTERVAL)
    return () => clearInterval(timer)
  }, [live])

  const refresh = () => setVersion((v) => v + 1)

  const setEnabled = (enabled: boolean) =>
    call<Status>('set_enabled', { enabled })
      .then((next) => {
        setStatus(next)
        if (enabled && next.permission === 'denied') return call<Status>('request_permission').then(setStatus)
      })
      .catch(fail)

  const requestPermission = () => call<Status>('request_permission').then(setStatus).catch(fail)

  const setExcluded = (ids: string[]) =>
    call('set_excluded', { apps: ids })
      .then(() => call<Status>('status'))
      .then((next) => {
        setStatus(next)
        refresh()
      })
      .catch(fail)

  const exclude = (id: string) => {
    if (!status) return
    setExcluded([...status.excluded.map((a) => a.id), id]).then(() => toast.success(t('apps.excluded')))
  }

  const clear = () =>
    call<Status>('clear')
      .then((next) => {
        setStatus(next)
        refresh()
        toast.success(t('settings.cleared'))
      })
      .catch(fail)

  if (!status) {
    return (
      <div className="flex h-full items-center justify-center text-fg-muted">
        {error ? <p className="text-[13px] text-danger">{error}</p> : <Spinner />}
      </div>
    )
  }

  const platform = status.os === 'macos' ? 'mac' : 'pc'
  const data = summary?.range === range ? summary.data : null
  const welcome = !status.enabled && !status.unsupported && data?.firstDay === null && tab !== 'settings'

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <Tabs
          value={tab}
          onValueChange={setTab}
          items={[
            { value: 'overview', label: t('tabs.overview'), icon: <Gauge /> },
            { value: 'keyboard', label: t('tabs.keyboard'), icon: <KeyboardIcon /> },
            { value: 'mouse', label: t('tabs.mouse'), icon: <MouseIcon /> },
            { value: 'apps', label: t('tabs.apps'), icon: <AppWindow /> },
            { value: 'settings', label: t('tabs.settings'), icon: <Settings2 /> },
          ]}
          aria-label={t('name')}
        />
        <div className="ml-auto flex items-center gap-3">
          {tab !== 'settings' && (
            <SegmentedControl
              size="sm"
              value={range}
              onValueChange={setRange}
              options={RANGES.map((value) => ({ value, label: t(`range.${value}`) }))}
              aria-label={t('range.label')}
            />
          )}
          <StatusBadge status={status} />
        </div>
      </div>

      {status.unsupported && (
        <div className="flex items-start gap-2 rounded-card border border-border bg-warning-soft px-3 py-2.5 text-[13px] text-fg">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warning" />
          <span>{t(`unsupported.${status.unsupported}`, { defaultValue: t('unsupported.other') })}</span>
        </div>
      )}
      {status.enabled && status.permission === 'denied' && (
        <div className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-warning-soft px-3 py-2.5 text-[13px] text-fg">
          <ShieldAlert className="size-4 shrink-0 text-warning" />
          <span className="min-w-0 flex-1">{t('permission.message')}</span>
          <Button size="sm" variant="primary" onClick={requestPermission}>
            {t('settings.grant')}
          </Button>
        </div>
      )}
      {status.enabled && status.error && status.error !== 'input_stats.permission' && (
        <p className="text-[13px] text-danger">{errorMessage({ code: status.error })}</p>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {tab === 'settings' ? (
          <Settings
            status={status}
            onEnable={setEnabled}
            onPermission={requestPermission}
            onInclude={(id) => setExcluded(status.excluded.filter((a) => a.id !== id).map((a) => a.id))}
            onClear={clear}
          />
        ) : welcome ? (
          <Empty
            icon={<KeyboardIcon />}
            title={t('welcome.title')}
            description={t('welcome.description')}
            action={
              <Button variant="primary" onClick={() => setEnabled(true)}>
                {t('welcome.start')}
              </Button>
            }
            className="h-full"
          />
        ) : !data ? (
          <div className="flex h-full items-center justify-center text-fg-muted">
            <Spinner />
          </div>
        ) : tab === 'overview' ? (
          <Overview summary={data} />
        ) : tab === 'keyboard' ? (
          <Keyboard summary={data} platform={platform} os={status.os} />
        ) : tab === 'mouse' ? (
          <Mouse summary={data} range={range} version={version} />
        ) : (
          <Apps summary={data} onExclude={exclude} />
        )}
      </div>
    </div>
  )
}

export default InputStats
