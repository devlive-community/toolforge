import { Badge, Button, Switch, Tooltip, toast } from '@toolforge/ui'
import { host, useCopy, useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { Check, Copy, FolderPlus, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Row, Section } from './Page'

interface AgentPath {
  path: string
  access: 'read' | 'write'
}

interface AgentOverview {
  enabled: boolean
  command: string | null
  config: string | null
  paths: AgentPath[]
}

/** 设置页「AI Agent」：只放全局项（总开关、MCP 接入、授权目录）；每个工具的开关在工具市场里配置 */
export function AgentSection() {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage()
  const { copy, copied } = useCopy()
  const [overview, setOverview] = useState<AgentOverview | null>(null)

  useEffect(() => {
    invoke<AgentOverview>('agent_overview')
      .then(setOverview)
      .catch(() => {})
  }, [])

  const apply = async (command: string, args: Record<string, unknown>) => {
    try {
      setOverview(await invoke<AgentOverview>(command, args))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const addPath = async (write: boolean) => {
    const path = await host.dialog.openDirectory()
    if (path) await apply('agent_add_path', { path, write })
  }

  if (!overview) return null
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

            <p className="px-5 py-3 text-xs text-fg-muted">{t('settings.agent.perTool')}</p>
          </>
        )}
      </div>
    </Section>
  )
}
