import { Fragment, useEffect, useState, type ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge, Button, Drawer } from '@toolforge/ui'
import { Settings2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'

interface InterfaceInfo {
  function: string
  summary: string
  channels: ('cli' | 'agent')[]
  effect: 'read' | 'write' | 'external'
}

/** 插件开放给命令行与 AI Agent 的函数；声明由 Rust 解析，未安装的在线插件没有 */
function InterfacesSection({ pluginId, onConfigure }: { pluginId: string; onConfigure?: () => void }) {
  const { t } = useTranslation()
  const [items, setItems] = useState<InterfaceInfo[]>([])

  useEffect(() => {
    invoke<InterfaceInfo[]>('plugin_interfaces', { pluginId })
      .then(setItems)
      .catch(() => setItems([]))
  }, [pluginId])

  if (items.length === 0) return null
  return (
    <section className="space-y-2">
      <h3 className="text-xs font-semibold text-fg-muted">{t('market.interfaces.title')}</h3>
      <ul className="space-y-1.5">
        {items.map((item) => (
          <li key={item.function} className="flex flex-wrap items-center gap-1.5 text-xs">
            <span className="text-fg">{item.summary.startsWith('i18n:') ? t(item.summary.slice(5), { ns: pluginId }) : item.summary}</span>
            {item.channels.map((channel) => (
              <Badge key={channel} variant="info">
                {t(`market.interfaces.${channel}`)}
              </Badge>
            ))}
            <Badge>{t(`market.config.effect.${item.effect}`)}</Badge>
          </li>
        ))}
      </ul>
      {onConfigure && (
        <Button size="sm" variant="outline" onClick={onConfigure}>
          <Settings2 />
          {t('market.interfaces.manage')}
        </Button>
      )}
    </section>
  )
}

/** 详情抽屉的归一化插件信息：内置插件与在线插件都映射到这组字段 */
export interface PluginDetailInfo {
  id: string
  name: string
  description: string
  version: string
  category: string
  keywords?: string[]
  permissions?: string[]
  functionsCount?: number
  platforms?: string[]
  sensitive?: boolean
  /** 功能说明（来自插件 help 命名空间；未安装的在线插件没有） */
  helpLines?: string[]
}

export interface PluginDetailRow {
  label: string
  value: ReactNode
}

interface PluginDetailProps {
  info: PluginDetailInfo | null
  /** 图标由调用方渲染：内置插件用 ToolTile，在线插件用索引里的 SVG */
  icon: ReactNode
  /** 来源徽标（内置 / 官方签名…） */
  originBadge?: ReactNode
  /** 操作区（打开 / 安装 / 卸载…） */
  actions?: ReactNode
  /** 详细信息表的附加行（包大小、应用要求、安装时间…） */
  extraRows?: PluginDetailRow[]
  /** 打开工具配置弹窗（命令行与 AI Agent）；没有时不显示入口 */
  onConfigure?: () => void
  onClose: () => void
}

/** 插件详情抽屉：“全部工具”与“在线插件”共用，保证两个列表的详情体验一致 */
export function PluginDetail({ info, icon, originBadge, actions, extraRows, onConfigure, onClose }: PluginDetailProps) {
  const { t } = useTranslation()
  return (
    <Drawer open={info !== null} onOpenChange={(open) => !open && onClose()} title={info?.name ?? ''} closeLabel={t('common:close')}>
      {info && (
        <div className="space-y-5 p-4">
          <div className="flex items-start gap-3">
            {icon}
            <div className="min-w-0 space-y-1.5">
              <p className="text-[13px] leading-relaxed text-fg-muted">{info.description}</p>
              <div className="flex flex-wrap gap-1.5">
                <Badge>{t('market.version', { version: info.version })}</Badge>
                <Badge>{t(`category.${info.category}`, { defaultValue: info.category })}</Badge>
                {originBadge}
                {info.sensitive && <Badge variant="warning">{t('market.sensitive')}</Badge>}
              </div>
            </div>
          </div>
          {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
          {info.helpLines && info.helpLines.length > 0 && (
            <section className="space-y-2">
              <h3 className="text-xs font-semibold text-fg-muted">{t('market.features')}</h3>
              <ul className="list-disc space-y-1.5 pl-4 text-[13px] leading-relaxed text-fg">
                {info.helpLines.map((line, index) => (
                  <li key={index}>{line}</li>
                ))}
              </ul>
            </section>
          )}
          <InterfacesSection pluginId={info.id} onConfigure={onConfigure} />
          <section className="space-y-2 text-xs">
            <h3 className="font-semibold text-fg-muted">{t('market.details')}</h3>
            <dl className="grid grid-cols-[96px_minmax(0,1fr)] gap-x-3 gap-y-1.5">
              <dt className="text-fg-subtle">{t('market.id')}</dt>
              <dd className="font-mono break-all text-fg" data-selectable>
                {info.id}
              </dd>
              {info.functionsCount !== undefined && (
                <>
                  <dt className="text-fg-subtle">{t('market.functions')}</dt>
                  <dd className="text-fg">{info.functionsCount}</dd>
                </>
              )}
              <dt className="text-fg-subtle">{t('market.permissions')}</dt>
              <dd className="flex flex-wrap gap-1">
                {(info.permissions ?? []).length === 0 ? (
                  <span className="text-fg-muted">{t('market.none')}</span>
                ) : (
                  info.permissions?.map((permission) => (
                    <code key={permission} className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-fg">
                      {permission}
                    </code>
                  ))
                )}
              </dd>
              <dt className="text-fg-subtle">{t('market.platforms')}</dt>
              <dd className="text-fg">{info.platforms?.length ? info.platforms.join(' / ') : t('market.allPlatforms')}</dd>
              {info.keywords && info.keywords.length > 0 && (
                <>
                  <dt className="text-fg-subtle">{t('market.keywords')}</dt>
                  <dd className="text-fg-muted">{info.keywords.join(' · ')}</dd>
                </>
              )}
              {extraRows?.map((row) => (
                <Fragment key={row.label}>
                  <dt className="text-fg-subtle">{row.label}</dt>
                  <dd className="text-fg">{row.value}</dd>
                </Fragment>
              ))}
            </dl>
          </section>
        </div>
      )}
    </Drawer>
  )
}
