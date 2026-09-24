import type { ReactNode } from 'react'
import { Badge, Panel, cn } from '@toolforge/ui'
import { formatBytes, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDown, ArrowUp, Cpu, HardDrive, MemoryStick, Monitor, Network, Thermometer } from 'lucide-react'
import type { Snapshot } from './types'

const percent = (used: number, total: number) => (total > 0 ? (used / total) * 100 : 0)

function tone(value: number) {
  if (value >= 90) return 'bg-danger'
  if (value >= 70) return 'bg-warning'
  return 'bg-primary'
}

function Bar({ value, className }: { value: number; className?: string }) {
  return (
    <div className={cn('h-1.5 overflow-hidden rounded-full bg-surface-2', className)}>
      <div className={cn('h-full rounded-full transition-[width] duration-500', tone(value))} style={{ width: `${Math.min(100, Math.max(0, value))}%` }} />
    </div>
  )
}

/** 单个核心的占用：自下而上填充 */
function CoreBar({ value }: { value: number }) {
  return (
    <div className="flex h-8 flex-col justify-end overflow-hidden rounded-sm bg-surface-2">
      <div className={cn('transition-[height] duration-500', tone(value))} style={{ height: `${Math.min(100, Math.max(0, value))}%` }} />
    </div>
  )
}

/** 最近一段时间的占用曲线 */
function Sparkline({ values }: { values: number[] }) {
  if (values.length < 2) return <div className="h-14" />
  const width = 240
  const step = width / (values.length - 1)
  const points = values.map((v, i) => `${(i * step).toFixed(1)},${(56 - (Math.min(100, v) / 100) * 52).toFixed(1)}`).join(' ')
  return (
    <svg viewBox={`0 0 ${width} 56`} preserveAspectRatio="none" className="h-14 w-full" aria-hidden>
      <polygon points={`0,56 ${points} ${width},56`} className="fill-primary/15" />
      <polyline points={points} className="fill-none stroke-primary" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  )
}

function Stat({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="min-w-0">
      <p className="text-[11px] text-fg-subtle">{label}</p>
      <p className="truncate text-[13px] font-medium text-fg tabular-nums" data-selectable>
        {value}
      </p>
    </div>
  )
}

export function Overview({ snapshot, history }: { snapshot: Snapshot; history: { cpu: number[]; memory: number[] } }) {
  const { t } = usePlugin()
  const { os, cpu, memory, disks, networks, sensors } = snapshot
  const memoryPercent = percent(memory.used, memory.total)
  const uptime = {
    days: Math.floor(os.uptime / 86400),
    hours: Math.floor((os.uptime % 86400) / 3600),
    minutes: Math.floor((os.uptime % 3600) / 60),
  }

  return (
    <div className="grid h-full min-h-0 auto-rows-min grid-cols-2 gap-3 overflow-auto pb-1 xl:grid-cols-3">
      <Panel icon={<Monitor />} title={t('os.title')} bodyClassName="grid grid-cols-2 gap-3 p-4">
        <Stat label={t('os.hostname')} value={os.hostname ?? '-'} />
        <Stat label={t('os.system')} value={os.longVersion ?? os.name ?? '-'} />
        <Stat label={t('os.kernel')} value={os.kernel ?? '-'} />
        <Stat label={t('os.arch')} value={os.arch} />
        <Stat label={t('os.uptime')} value={t('os.uptimeValue', uptime)} />
        <Stat label={t('os.boot')} value={new Date(os.bootTime * 1000).toLocaleString()} />
      </Panel>

      <Panel
        icon={<Cpu />}
        title={t('cpu.title')}
        extra={<Badge variant={cpu.usage >= 90 ? 'danger' : cpu.usage >= 70 ? 'warning' : 'success'}>{`${cpu.usage.toFixed(1)}%`}</Badge>}
        bodyClassName="space-y-3 p-4"
      >
        <p className="truncate text-xs text-fg-muted" title={cpu.brand}>
          {t('cpu.summary', { brand: cpu.brand || cpu.vendor, physical: cpu.physicalCores ?? '-', logical: cpu.logicalCores, ghz: (cpu.frequency / 1000).toFixed(2) })}
        </p>
        <Sparkline values={history.cpu} />
        <div className="grid gap-1.5" style={{ gridTemplateColumns: `repeat(${Math.min(12, Math.max(1, cpu.cores.length))}, minmax(0, 1fr))` }}>
          {cpu.cores.map((usage, index) => (
            <div key={index} title={t('cpu.core', { index: index + 1, usage: usage.toFixed(0) })} className="space-y-1">
              <CoreBar value={usage} />
              <p className="text-center text-[10px] text-fg-subtle tabular-nums">{usage.toFixed(0)}</p>
            </div>
          ))}
        </div>
        {cpu.load.some((v) => v > 0) && <p className="text-xs text-fg-muted tabular-nums">{t('cpu.load', { one: cpu.load[0].toFixed(2), five: cpu.load[1].toFixed(2), fifteen: cpu.load[2].toFixed(2) })}</p>}
      </Panel>

      <Panel
        icon={<MemoryStick />}
        title={t('memory.title')}
        extra={<Badge variant={memoryPercent >= 90 ? 'danger' : memoryPercent >= 70 ? 'warning' : 'success'}>{`${memoryPercent.toFixed(1)}%`}</Badge>}
        bodyClassName="space-y-3 p-4"
      >
        <Sparkline values={history.memory} />
        <div className="space-y-1.5">
          <p className="flex justify-between text-xs text-fg-muted tabular-nums">
            <span>{t('memory.used')}</span>
            <span>{`${formatBytes(memory.used)} / ${formatBytes(memory.total)}`}</span>
          </p>
          <Bar value={memoryPercent} />
        </div>
        {memory.swapTotal > 0 && (
          <div className="space-y-1.5">
            <p className="flex justify-between text-xs text-fg-muted tabular-nums">
              <span>{t('memory.swap')}</span>
              <span>{`${formatBytes(memory.swapUsed)} / ${formatBytes(memory.swapTotal)}`}</span>
            </p>
            <Bar value={percent(memory.swapUsed, memory.swapTotal)} />
          </div>
        )}
        <p className="text-xs text-fg-subtle">{t('memory.available', { size: formatBytes(memory.available) })}</p>
      </Panel>

      <Panel icon={<HardDrive />} title={t('disks.title')} extra={<Badge>{disks.length}</Badge>} className="xl:col-span-1" bodyClassName="space-y-3 p-4">
        {disks.length === 0 && <p className="text-xs text-fg-subtle">{t('disks.empty')}</p>}
        {disks.map((disk) => {
          const used = disk.total - disk.available
          return (
            <div key={`${disk.name}-${disk.mount}`} className="space-y-1.5">
              <div className="flex items-center gap-2 text-xs">
                <span className="min-w-0 flex-1 truncate font-medium text-fg" title={disk.mount} data-selectable>
                  {disk.mount}
                </span>
                <span className="shrink-0 text-fg-subtle">{[disk.fileSystem, disk.kind !== 'unknown' ? disk.kind.toUpperCase() : null, disk.removable ? t('disks.removable') : null].filter(Boolean).join(' · ')}</span>
              </div>
              <Bar value={percent(used, disk.total)} />
              <p className="text-[11px] text-fg-subtle tabular-nums">{t('disks.usage', { used: formatBytes(used), total: formatBytes(disk.total), free: formatBytes(disk.available) })}</p>
            </div>
          )
        })}
      </Panel>

      <Panel icon={<Network />} title={t('network.title')} className="col-span-2 xl:col-span-2" bodyClassName="divide-y divide-border">
        {networks.slice(0, 8).map((net) => (
          <div key={net.name} className="grid grid-cols-[8rem_minmax(0,1fr)_9rem_9rem] items-center gap-3 px-4 py-2.5 text-xs">
            <div className="min-w-0">
              <p className="truncate font-medium text-fg">{net.name}</p>
              {net.mac && <p className="truncate font-mono text-[11px] text-fg-subtle">{net.mac}</p>}
            </div>
            <p className="truncate font-mono text-fg-muted" title={net.addresses.join('\n')} data-selectable>
              {net.addresses.join('  ') || '-'}
            </p>
            <p className="flex items-center gap-1 text-fg tabular-nums">
              <ArrowDown className="size-3.5 text-success" />
              {`${formatBytes(net.rxRate)}/s`}
            </p>
            <p className="flex items-center gap-1 text-fg tabular-nums">
              <ArrowUp className="size-3.5 text-info" />
              {`${formatBytes(net.txRate)}/s`}
            </p>
          </div>
        ))}
      </Panel>

      {sensors.length > 0 && (
        <Panel icon={<Thermometer />} title={t('sensors.title')} bodyClassName="grid grid-cols-2 gap-3 p-4">
          {sensors.slice(0, 12).map((sensor) => (
            <Stat key={sensor.label} label={sensor.label} value={`${sensor.temperature.toFixed(1)} °C`} />
          ))}
        </Panel>
      )}
    </div>
  )
}
