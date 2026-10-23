import { useEffect, useMemo, useState } from 'react'
import { Badge, Button, Empty, Input, SegmentedControl, Select, Switch, Tabs, Tooltip, cn, toast } from '@toolforge/ui'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { CloudDownload, Eye, EyeOff, LayoutGrid, PackageSearch, Search, Settings2, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { CATEGORIES } from '../lib/categories'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { useApp } from '../stores/app'
import { toolStatus, usePrefs, type ToolStatus } from '../stores/prefs'
import { MarketOnline } from './MarketOnline'
import { Page } from './Page'
import { PluginConfigDialog, loadPluginConfig, type PluginInterfaceConfig } from './PluginConfig'
import { PluginDetail } from './PluginDetail'

type Filter = 'all' | 'changed' | 'hidden'

const STATUS_VARIANT: Record<Exclude<ToolStatus, 'seen'>, 'success' | 'info'> = { new: 'success', updated: 'info' }

function matches(manifest: Manifest, query: string, text: (m: Manifest, v: string) => string) {
  if (!query) return true
  const haystack = [text(manifest, manifest.name), text(manifest, manifest.description), manifest.id, ...manifest.keywords].join(' ').toLowerCase()
  return query
    .toLowerCase()
    .split(/\s+/)
    .every((word) => haystack.includes(word))
}

/** 工具卡片的操作：全部是图标按钮 + Tooltip；不可用的操作置灰并说明原因 */
function ToolActions({
  name,
  supported,
  hidden,
  removable,
  busy,
  onConfigure,
  onToggleHidden,
  onUninstall,
}: {
  name: string
  supported: boolean
  hidden: boolean
  removable: boolean
  busy: boolean
  onConfigure: () => void
  onToggleHidden: () => void
  onUninstall: () => void
}) {
  const { t } = useTranslation()
  const configureLabel = supported ? t('market.actions.configure', { name }) : t('market.actions.configureUnsupported')
  const hideLabel = hidden ? t('market.actions.show', { name }) : t('market.actions.hide', { name })
  const uninstallLabel = removable ? t('market.actions.uninstall', { name }) : t('market.actions.uninstallBuiltin')
  return (
    <div className="ml-auto flex items-center gap-0.5">
      <Tooltip content={configureLabel}>
        {/* 禁用的按钮不触发指针事件，包一层才能显示提示 */}
        <span className="inline-flex">
          <Button size="icon-sm" variant="ghost" aria-label={configureLabel} disabled={!supported} onClick={onConfigure}>
            <Settings2 />
          </Button>
        </span>
      </Tooltip>
      <Tooltip content={hideLabel}>
        <Button size="icon-sm" variant="ghost" aria-label={hideLabel} onClick={onToggleHidden}>
          {hidden ? <EyeOff /> : <Eye />}
        </Button>
      </Tooltip>
      <Tooltip content={uninstallLabel}>
        <span className="inline-flex">
          <Button size="icon-sm" variant="ghost" aria-label={uninstallLabel} disabled={!removable || busy} onClick={onUninstall}>
            <Trash2 />
          </Button>
        </span>
      </Tooltip>
    </div>
  )
}

export function Market() {
  const { t } = useTranslation()
  const text = usePluginText()
  const plugins = useApp((s) => s.plugins)
  const openTool = useApp((s) => s.openTool)
  const hidden = usePrefs((s) => s.hiddenTools)
  const seen = usePrefs((s) => s.seenVersions)
  const setHidden = usePrefs((s) => s.setToolHidden)
  const markSeen = usePrefs((s) => s.markToolsSeen)
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('all')
  const [filter, setFilter] = useState<Filter>('all')
  const [selected, setSelected] = useState<Manifest | null>(null)
  const [tab, setTab] = useState<'tools' | 'online'>('tools')
  const errorMessage = useErrorMessage()
  /** 插件 id → 开放的通道（cli / agent）；没有声明的旧插件不在其中 */
  const [support, setSupport] = useState<Record<string, string[]>>({})
  /** 在线安装的插件（可以卸载）；内置插件不在其中 */
  const [installedIds, setInstalledIds] = useState<string[]>([])
  const [configuring, setConfiguring] = useState<{ manifest: Manifest; config: PluginInterfaceConfig } | null>(null)
  const [busy, setBusy] = useState<string | null>(null)

  useEffect(() => {
    invoke<Record<string, string[]>>('interface_support')
      .then(setSupport)
      .catch(() => {})
    invoke<{ id: string }[]>('plugin_installed')
      .then((list) => setInstalledIds(list.map((info) => info.id)))
      .catch(() => {})
  }, [plugins])

  const configure = async (manifest: Manifest) => {
    try {
      setConfiguring({ manifest, config: await loadPluginConfig(manifest.id) })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const uninstall = async (manifest: Manifest) => {
    setBusy(manifest.id)
    try {
      await invoke('plugin_uninstall', { pluginId: manifest.id })
      toast.success(t('market.online.uninstalled', { name: text(manifest, manifest.name), id: manifest.id }))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(null)
    }
  }

  const changed = plugins.filter((p) => toolStatus(p, seen) !== 'seen')
  const list = useMemo(
    () =>
      plugins
        .filter((p) => category === 'all' || p.category === category)
        .filter((p) => filter !== 'changed' || toolStatus(p, seen) !== 'seen')
        .filter((p) => filter !== 'hidden' || hidden.includes(p.id))
        .filter((p) => matches(p, query.trim(), text))
        .sort((a, b) => text(a, a.name).localeCompare(text(b, b.name))),
    [plugins, category, filter, hidden, seen, query, text],
  )

  // 详情抽屉的功能说明来自选中插件的 help 命名空间
  const help = selected ? t('help', { ns: selected.id, returnObjects: true, defaultValue: [] }) : []
  const helpLines = Array.isArray(help) ? (help as string[]) : []

  return (
    <Page title={t('nav.market')} description={t('market.subtitle', { count: plugins.length })}>
      <div className="mb-5">
        <Tabs
          value={tab}
          onValueChange={setTab}
          items={[
            { value: 'tools', label: t('market.tabs.tools'), icon: <LayoutGrid /> },
            { value: 'online', label: t('market.tabs.online'), icon: <CloudDownload /> },
          ]}
        />
      </div>
      {tab === 'online' ? (
        <MarketOnline />
      ) : (
        <>
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t('market.search')} aria-label={t('market.search')} leading={<Search />} wrapperClassName="min-w-60 flex-1" />
        <Select<string>
          value={category}
          onValueChange={setCategory}
          className="w-36"
          aria-label={t('market.category')}
          options={[{ value: 'all', label: t('market.allCategories') }, ...CATEGORIES.map(({ id }) => ({ value: id, label: t(`category.${id}`) }))]}
        />
        <SegmentedControl<Filter>
          value={filter}
          onValueChange={setFilter}
          aria-label={t('market.filter')}
          options={[
            { value: 'all', label: t('market.filters.all') },
            { value: 'changed', label: t('market.filters.changed', { count: changed.length }) },
            { value: 'hidden', label: t('market.filters.hidden', { count: hidden.length }) },
          ]}
        />
        {changed.length > 0 && (
          <Button variant="ghost" onClick={() => markSeen(plugins)}>
            {t('market.markSeen')}
          </Button>
        )}
      </div>
      {list.length === 0 ? (
        <Empty className="py-24" icon={<PackageSearch />} title={t('market.empty')} />
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3">
          {list.map((manifest) => {
            const status = toolStatus(manifest, seen)
            const isHidden = hidden.includes(manifest.id)
            return (
              <div key={manifest.id} className={cn('flex flex-col gap-3 rounded-card border border-border bg-surface p-4 shadow-card', isHidden && 'opacity-60')}>
                <button type="button" onClick={() => setSelected(manifest)} className="flex items-start gap-3 rounded-control text-left outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  <ToolTile manifest={manifest} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <p className="truncate text-sm font-semibold text-fg">{text(manifest, manifest.name)}</p>
                      {status !== 'seen' && <Badge variant={STATUS_VARIANT[status]}>{t(`market.status.${status}`)}</Badge>}
                    </div>
                    <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-fg-muted">{text(manifest, manifest.description)}</p>
                  </div>
                </button>
                <div className="mt-auto flex items-center gap-2 text-[11px] text-fg-subtle">
                  <span>{t('market.version', { version: manifest.version })}</span>
                  <span>·</span>
                  <span>{t(`category.${manifest.category}`, { defaultValue: manifest.category })}</span>
                  <ToolActions
                    name={text(manifest, manifest.name)}
                    supported={(support[manifest.id] ?? []).length > 0}
                    hidden={isHidden}
                    removable={installedIds.includes(manifest.id)}
                    busy={busy === manifest.id}
                    onConfigure={() => void configure(manifest)}
                    onToggleHidden={() => setHidden(manifest.id, !isHidden)}
                    onUninstall={() => void uninstall(manifest)}
                  />
                </div>
              </div>
            )
          })}
        </div>
      )}
        </>
      )}
      <PluginDetail
        info={
          selected && {
            id: selected.id,
            name: text(selected, selected.name),
            description: text(selected, selected.description),
            version: selected.version,
            category: selected.category,
            keywords: selected.keywords,
            permissions: selected.permissions,
            functionsCount: Object.keys(selected.functions).length,
            platforms: selected.platforms,
            sensitive: selected.sensitive,
            helpLines,
          }
        }
        icon={selected && <ToolTile manifest={selected} size="lg" />}
        originBadge={<Badge variant="success">{t('market.builtin')}</Badge>}
        actions={
          selected && (
            <>
              <Button
                variant="primary"
                onClick={() => {
                  setSelected(null)
                  openTool(selected.id)
                }}
              >
                <SquareArrowOutUpRight />
                {t('market.open')}
              </Button>
              <label className="ml-auto flex items-center gap-2 text-[13px] text-fg">
                <Switch size="sm" checked={!hidden.includes(selected.id)} onCheckedChange={(on) => setHidden(selected.id, !on)} aria-label={t('market.visible')} />
                {t('market.visible')}
              </label>
            </>
          )
        }
        onConfigure={
          selected && (support[selected.id] ?? []).length > 0
            ? () => {
                const manifest = selected
                setSelected(null)
                void configure(manifest)
              }
            : undefined
        }
        onClose={() => setSelected(null)}
      />
      <PluginConfigDialog
        manifest={configuring?.manifest ?? null}
        config={configuring?.config ?? null}
        onChange={(config) => setConfiguring((current) => (current ? { ...current, config } : current))}
        onClose={() => setConfiguring(null)}
      />
    </Page>
  )
}
