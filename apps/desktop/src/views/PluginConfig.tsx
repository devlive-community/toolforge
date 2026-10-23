import { Badge, Button, Empty, Modal, Select, Switch, Tabs, cn, toast } from '@toolforge/ui'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { Bot, ChevronRight, ShieldAlert, SquareTerminal } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { useApp } from '../stores/app'

type Mode = 'off' | 'ask' | 'allow'
type Channel = 'cli' | 'agent'

interface FunctionConfig {
  name: string
  summary: string
  effect: 'read' | 'write' | 'external'
  uses: string[]
  mode: Mode
  defaultMode: Mode
  custom: Mode | null
  capped: boolean
}

interface ChannelConfig {
  enabled: boolean
  ready: boolean
  functions: FunctionConfig[]
}

export interface PluginInterfaceConfig {
  id: string
  sensitive: boolean
  native: boolean
  cli: ChannelConfig | null
  agent: ChannelConfig | null
}

const EFFECT_BADGE: Record<FunctionConfig['effect'], 'neutral' | 'warning' | 'danger'> = { read: 'neutral', write: 'warning', external: 'danger' }

/** 读取配置后再打开弹窗，首帧即完整内容 */
export function loadPluginConfig(pluginId: string) {
  return invoke<PluginInterfaceConfig>('plugin_interface_config', { pluginId })
}

/**
 * 工具配置弹窗：用页签区分命令行与 AI Agent，按插件、按函数开关（默认全部开启）。
 * 插件自身的配置只在这里，不放进系统设置页
 */
export function PluginConfigDialog({ manifest, config, onChange, onClose }: { manifest: Manifest | null; config: PluginInterfaceConfig | null; onChange: (config: PluginInterfaceConfig) => void; onClose: () => void }) {
  const { t } = useTranslation()
  const text = usePluginText()
  const errorMessage = useErrorMessage()
  const navigate = useApp((s) => s.navigate)
  const [tab, setTab] = useState<Channel>('cli')
  const [busy, setBusy] = useState(false)
  const [sensitive, setSensitive] = useState(false)

  const open = manifest !== null && config !== null
  const name = manifest ? text(manifest, manifest.name) : ''
  const summary = (value: string) => (manifest && value.startsWith('i18n:') ? t(value.slice(5), { ns: manifest.id }) : value)

  const apply = async (command: 'interface_set_plugin' | 'interface_set_function', args: Record<string, unknown>) => {
    if (!config) return
    setBusy(true)
    try {
      onChange(await invoke<PluginInterfaceConfig>(command, { channelId: tab, pluginId: config.id, ...args }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  const togglePlugin = (enabled: boolean) => {
    if (enabled && tab === 'agent' && config?.sensitive) {
      setSensitive(true)
      return
    }
    void apply('interface_set_plugin', { enabled })
  }

  const modeLabel = (mode: Mode) => t(`market.config.mode.${mode}`)

  const renderPanel = (key: Channel) => {
    const channel = config?.[key] ?? null
    if (!channel) {
      return (
        <Empty
          icon={key === 'cli' ? <SquareTerminal /> : <Bot />}
          title={t(`market.config.unsupported.${key}`)}
          description={t('market.config.unsupportedHint')}
        />
      )
    }
    return (
      <div className="space-y-3">
        {!channel.ready && (
          <div className="flex items-center justify-between gap-3 rounded-control bg-warning-soft px-3 py-2">
            <p className="text-xs text-warning">{t(`market.config.notReady.${key}`)}</p>
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                onClose()
                navigate({ view: 'settings' })
              }}
            >
              {t('market.config.openSettings')}
              <ChevronRight />
            </Button>
          </div>
        )}
        <div className="flex items-center justify-between gap-4 rounded-control border border-border px-3 py-2.5">
          <div>
            <p className="text-[13px] font-medium text-fg">{t(`market.config.enable.${key}`)}</p>
            <p className="mt-0.5 text-xs text-fg-muted">{t(`market.config.enableHint.${key}`)}</p>
            {key === 'agent' && config?.native && <p className="mt-1 text-xs text-fg-subtle">{t('market.config.nativeHint')}</p>}
          </div>
          <Switch checked={channel.enabled} disabled={busy} onCheckedChange={togglePlugin} aria-label={t(`market.config.enable.${key}`)} />
        </div>
        <ul className="divide-y divide-border rounded-control border border-border">
          {channel.functions.map((fn) => (
            <li key={fn.name} className="flex items-center gap-3 px-3 py-2">
              <div className="min-w-0 flex-1">
                <p className="truncate text-xs text-fg">{summary(fn.summary)}</p>
                <div className="mt-1 flex flex-wrap items-center gap-1">
                  <code className="font-mono text-[11px] text-fg-subtle">{fn.name}</code>
                  <Badge variant={EFFECT_BADGE[fn.effect]}>{t(`market.config.effect.${fn.effect}`)}</Badge>
                  {fn.uses.map((use) => (
                    <code key={use} className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg-muted">
                      {use}
                    </code>
                  ))}
                  {fn.capped && <span className="text-[11px] text-fg-subtle">{t('market.config.capped')}</span>}
                </div>
              </div>
              {key === 'cli' ? (
                <Switch
                  size="sm"
                  checked={fn.mode !== 'off'}
                  disabled={busy || !channel.enabled}
                  onCheckedChange={(on) => void apply('interface_set_function', { function: fn.name, mode: on ? null : 'off' })}
                  aria-label={summary(fn.summary)}
                />
              ) : (
                <Select<'default' | Mode>
                  size="sm"
                  className="w-40 shrink-0"
                  value={fn.custom ?? 'default'}
                  disabled={busy || !channel.enabled}
                  aria-label={summary(fn.summary)}
                  options={[
                    { value: 'default', label: t('market.config.mode.default', { mode: modeLabel(fn.defaultMode) }) },
                    { value: 'off', label: modeLabel('off') },
                    { value: 'ask', label: modeLabel('ask') },
                    ...(fn.capped ? [] : [{ value: 'allow' as const, label: modeLabel('allow') }]),
                  ]}
                  onValueChange={(value) => void apply('interface_set_function', { function: fn.name, mode: value === 'default' ? null : value })}
                />
              )}
            </li>
          ))}
        </ul>
      </div>
    )
  }

  return (
    <>
      <Modal open={open} onOpenChange={(next) => !next && onClose()} title={t('market.config.title', { name })} closeLabel={t('common:close')} size="lg">
        <div className="space-y-4 px-5 pb-5">
          <Tabs<Channel>
            value={tab}
            onValueChange={setTab}
            items={[
              { value: 'cli', label: t('market.config.cli'), icon: <SquareTerminal /> },
              { value: 'agent', label: t('market.config.agent'), icon: <Bot /> },
            ]}
          />
          {/* 两个页签叠放在同一个网格格子里，未选中的不可见但占位：
              弹窗高度取两者中较高的一个，切换页签不跳动，也不出现内部滚动条 */}
          <div className="grid">
            {(['cli', 'agent'] as const).map((key) => (
              <div key={key} className={cn('[grid-area:1/1]', key !== tab && 'invisible')} aria-hidden={key !== tab}>
                {renderPanel(key)}
              </div>
            ))}
          </div>
        </div>
      </Modal>

      <Modal
        open={sensitive}
        onOpenChange={(next) => !next && setSensitive(false)}
        title={t('market.config.sensitiveTitle')}
        closeLabel={t('common:close')}
        footer={
          <>
            <Button onClick={() => setSensitive(false)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setSensitive(false)
                void apply('interface_set_plugin', { enabled: true })
              }}
            >
              <ShieldAlert />
              {t('market.config.sensitiveConfirm')}
            </Button>
          </>
        }
      >
        <p className="px-5 pb-5 text-[13px] text-fg">{t('market.config.sensitiveBody', { name })}</p>
      </Modal>
    </>
  )
}

/** 两个页签共用：哪些工具支持配置、读取配置后打开弹窗 */
export function useToolConfig(plugins: Manifest[]) {
  const errorMessage = useErrorMessage()
  /** 插件 id → 开放的通道（cli / agent）；没有声明的旧插件不在其中 */
  const [support, setSupport] = useState<Record<string, string[]>>({})
  const [configuring, setConfiguring] = useState<{ manifest: Manifest; config: PluginInterfaceConfig } | null>(null)

  useEffect(() => {
    invoke<Record<string, string[]>>('interface_support')
      .then(setSupport)
      .catch(() => {})
  }, [plugins])

  const configure = async (manifest: Manifest) => {
    try {
      setConfiguring({ manifest, config: await loadPluginConfig(manifest.id) })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const dialog = (
    <PluginConfigDialog
      manifest={configuring?.manifest ?? null}
      config={configuring?.config ?? null}
      onChange={(config) => setConfiguring((current) => (current ? { ...current, config } : current))}
      onClose={() => setConfiguring(null)}
    />
  )

  return {
    supported: (id: string) => (support[id] ?? []).length > 0,
    configure: (manifest: Manifest) => void configure(manifest),
    dialog,
  }
}
