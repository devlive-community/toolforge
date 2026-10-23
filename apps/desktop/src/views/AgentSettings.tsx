import { Badge, Button, Modal, SegmentedControl, Switch, Tooltip, toast } from '@toolforge/ui'
import { host, useCopy, useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { Check, ChevronDown, ChevronRight, Copy, FolderPlus, ShieldAlert, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { pluginById, useApp } from '../stores/app'
import { Row, Section } from './Page'

type Mode = 'off' | 'ask' | 'allow'
type Effect = 'read' | 'write' | 'external'

interface AgentFunction {
  name: string
  summary: string
  effect: Effect
  uses: string[]
  mode: Mode
  /** 没有单独设置时的模式 */
  defaultMode: Mode
  custom: Mode | null
  capped: boolean
}

interface AgentPlugin {
  id: string
  sensitive: boolean
  enabled: boolean
  native: boolean
  functions: AgentFunction[]
}

interface AgentPath {
  path: string
  access: 'read' | 'write'
}

interface AgentOverview {
  enabled: boolean
  command: string | null
  config: string | null
  paths: AgentPath[]
  plugins: AgentPlugin[]
}

const EFFECT_BADGE: Record<Effect, 'neutral' | 'warning' | 'danger'> = { read: 'neutral', write: 'warning', external: 'danger' }

/** 设置页「AI Agent」：总开关、MCP 接入配置、授权目录，以及按插件、按函数的授权；`tfp mcp` 每次调用时读取 */
export function AgentSection() {
  const { t } = useTranslation()
  const text = usePluginText()
  const errorMessage = useErrorMessage()
  const { copy, copied } = useCopy()
  const plugins = useApp((s) => s.plugins)
  const [overview, setOverview] = useState<AgentOverview | null>(null)
  const [expanded, setExpanded] = useState<string[]>([])
  const [sensitive, setSensitive] = useState<AgentPlugin | null>(null)

  useEffect(() => {
    invoke<AgentOverview>('agent_overview')
      .then(setOverview)
      .catch(() => {})
  }, [plugins])

  const apply = async (command: string, args: Record<string, unknown>) => {
    try {
      setOverview(await invoke<AgentOverview>(command, args))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const pluginName = (id: string) => {
    const manifest = pluginById(plugins, id)
    return manifest ? text(manifest, manifest.name) : id
  }
  const summary = (id: string, value: string) => (value.startsWith('i18n:') ? t(value.slice(5), { ns: id }) : value)

  const togglePlugin = (plugin: AgentPlugin, enabled: boolean) => {
    if (enabled && plugin.sensitive) {
      setSensitive(plugin)
      return
    }
    void apply('agent_set_plugin', { pluginId: plugin.id, enabled })
  }

  const addPath = async (write: boolean) => {
    const path = await host.dialog.openDirectory()
    if (path) await apply('agent_add_path', { path, write })
  }

  if (!overview) return null
  const modeLabel = (mode: Mode) => t(`settings.agent.mode.${mode}`)

  return (
    <Section title={t('settings.agent.title')}>
      <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
        <Row label={t('settings.agent.enabled')} hint={t('settings.agent.enabledHint')}>
          <Switch
            checked={overview.enabled}
            onCheckedChange={(enabled) => void apply('agent_set_enabled', { enabled })}
            aria-label={t('settings.agent.enabled')}
          />
        </Row>

        {overview.enabled && (
          <>
            <div className="space-y-2 px-5 py-4">
              <p className="text-[13px] font-medium text-fg">{t('settings.agent.connect')}</p>
              {overview.config ? (
                <>
                  <p className="text-xs text-fg-muted">{t('settings.agent.connectHint')}</p>
                  <div className="flex items-start gap-2 rounded-control border border-border bg-surface-2 px-3 py-2">
                    <pre className="min-w-0 flex-1 overflow-x-auto font-mono text-xs text-fg" data-selectable>
                      {overview.config}
                    </pre>
                    <Tooltip content={t('settings.agent.copy')}>
                      <Button size="icon-sm" variant="ghost" aria-label={t('settings.agent.copy')} onClick={() => void copy(overview.config ?? '')}>
                        {copied ? <Check className="text-success" /> : <Copy />}
                      </Button>
                    </Tooltip>
                  </div>
                </>
              ) : (
                <p className="text-xs text-warning">{t('settings.agent.unavailable')}</p>
              )}
            </div>

            <div className="space-y-2 px-5 py-4">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <p className="text-[13px] font-medium text-fg">{t('settings.agent.folders')}</p>
                  <p className="mt-0.5 text-xs text-fg-muted">{t('settings.agent.foldersHint')}</p>
                </div>
                <div className="flex shrink-0 gap-2">
                  <Button size="sm" onClick={() => void addPath(false)}>
                    <FolderPlus />
                    {t('settings.agent.addRead')}
                  </Button>
                  <Button size="sm" onClick={() => void addPath(true)}>
                    <FolderPlus />
                    {t('settings.agent.addWrite')}
                  </Button>
                </div>
              </div>
              {overview.paths.length === 0 ? (
                <p className="text-xs text-fg-subtle">{t('settings.agent.noFolders')}</p>
              ) : (
                <ul className="space-y-1">
                  {overview.paths.map((entry) => (
                    <li key={entry.path} className="flex items-center gap-2 rounded-control bg-surface-2 px-3 py-1.5">
                      <code className="min-w-0 flex-1 truncate font-mono text-xs text-fg" data-selectable>
                        {entry.path}
                      </code>
                      <Badge variant={entry.access === 'write' ? 'warning' : 'neutral'}>{t(`settings.agent.${entry.access}`)}</Badge>
                      <Tooltip content={t('settings.agent.remove')}>
                        <Button size="icon-sm" variant="ghost" aria-label={t('settings.agent.remove')} onClick={() => void apply('agent_remove_path', { path: entry.path })}>
                          <Trash2 />
                        </Button>
                      </Tooltip>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <div className="space-y-2 px-5 py-4">
              <p className="text-[13px] font-medium text-fg">{t('settings.agent.plugins')}</p>
              <p className="text-xs text-fg-muted">{t('settings.agent.pluginsHint')}</p>
              {overview.plugins.length === 0 && <p className="text-xs text-fg-subtle">{t('settings.agent.noPlugins')}</p>}
              <ul className="space-y-2">
                {overview.plugins.map((plugin) => {
                  const open = plugin.enabled && expanded.includes(plugin.id)
                  const toggleLabel = open ? t('settings.agent.collapse') : t('settings.agent.expand')
                  return (
                    <li key={plugin.id} className="rounded-control border border-border">
                      <div className="flex items-center gap-2 px-3 py-2">
                        <Tooltip content={toggleLabel}>
                          <Button
                            size="icon-sm"
                            variant="ghost"
                            aria-label={toggleLabel}
                            disabled={!plugin.enabled}
                            onClick={() => setExpanded((list) => (open ? list.filter((id) => id !== plugin.id) : [...list, plugin.id]))}
                          >
                            {open ? <ChevronDown /> : <ChevronRight />}
                          </Button>
                        </Tooltip>
                        <p className="min-w-0 flex-1 truncate text-[13px] text-fg">{pluginName(plugin.id)}</p>
                        {plugin.sensitive && <Badge variant="warning">{t('market.sensitive')}</Badge>}
                        {plugin.native && (
                          <Tooltip content={t('settings.agent.nativeHint')}>
                            <Badge>{t('settings.agent.native')}</Badge>
                          </Tooltip>
                        )}
                        <Switch
                          size="sm"
                          checked={plugin.enabled}
                          onCheckedChange={(enabled) => togglePlugin(plugin, enabled)}
                          aria-label={pluginName(plugin.id)}
                        />
                      </div>
                      {open && (
                        <ul className="divide-y divide-border border-t border-border">
                          {plugin.functions.map((fn) => {
                            const options = [
                              { value: 'default' as const, label: t('settings.agent.mode.default', { mode: modeLabel(fn.defaultMode) }) },
                              { value: 'off' as const, label: modeLabel('off') },
                              { value: 'ask' as const, label: modeLabel('ask') },
                              ...(fn.capped ? [] : [{ value: 'allow' as const, label: modeLabel('allow') }]),
                            ]
                            return (
                              <li key={fn.name} className="flex flex-wrap items-center gap-3 px-3 py-2">
                                <div className="min-w-0 flex-1">
                                  <p className="text-xs text-fg">{summary(plugin.id, fn.summary)}</p>
                                  <div className="mt-1 flex flex-wrap items-center gap-1">
                                    <code className="font-mono text-[11px] text-fg-subtle">{fn.name}</code>
                                    <Badge variant={EFFECT_BADGE[fn.effect]}>{t(`settings.agent.effect.${fn.effect}`)}</Badge>
                                    {fn.uses.map((use) => (
                                      <code key={use} className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-fg-muted">
                                        {use}
                                      </code>
                                    ))}
                                    {fn.capped && <span className="text-[11px] text-fg-subtle">{t('settings.agent.capped')}</span>}
                                  </div>
                                </div>
                                <SegmentedControl
                                  size="sm"
                                  value={fn.custom ?? 'default'}
                                  options={options}
                                  aria-label={summary(plugin.id, fn.summary)}
                                  onValueChange={(value) =>
                                    void apply('agent_set_function', { pluginId: plugin.id, function: fn.name, mode: value === 'default' ? null : value })
                                  }
                                />
                              </li>
                            )
                          })}
                        </ul>
                      )}
                    </li>
                  )
                })}
              </ul>
            </div>
          </>
        )}
      </div>

      <Modal
        open={sensitive !== null}
        onOpenChange={(open) => !open && setSensitive(null)}
        title={t('settings.agent.sensitiveTitle')}
        closeLabel={t('common:close')}
        footer={
          <>
            <Button onClick={() => setSensitive(null)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                const plugin = sensitive
                setSensitive(null)
                if (plugin) void apply('agent_set_plugin', { pluginId: plugin.id, enabled: true })
              }}
            >
              <ShieldAlert />
              {t('settings.agent.sensitiveConfirm')}
            </Button>
          </>
        }
      >
        {sensitive && <p className="text-[13px] text-fg">{t('settings.agent.sensitiveBody', { name: pluginName(sensitive.id) })}</p>}
      </Modal>
    </Section>
  )
}
