import { useCallback, useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge, Button, Empty, Modal, Spinner, cn, toast } from '@toolforge/ui'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { host, useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { CloudDownload, FileUp, PackageCheck, RefreshCw, ShieldAlert, ShieldCheck, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { ToolTile, accentTiles } from '../plugins/ToolIcon'
import { pluginById, useApp } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { useUpdate } from '../stores/update'
import { useToolConfig } from './PluginConfig'
import { PluginDetail, type PluginDetailInfo, type PluginDetailRow } from './PluginDetail'
import { ToolActions } from './ToolActions'
import { formatDownloadCount, loadDownloadStats, type PluginDownloadStats } from '../lib/pluginStats'
import { createInstallQueue } from '../lib/installQueue'

interface IndexEntry {
  id: string
  version: string
  category: string
  /** 图标底色（与内置工具 manifest.accent 同一规则） */
  accent?: string | null
  names: Record<string, string>
  descriptions: Record<string, string>
  icon: string | null
  url: string
  sha256: string
  size: number
  signed: boolean
  /** 插件需要的最低应用版本 */
  minAppVersion?: string | null
}

interface InstallInfo {
  id: string
  version: string
  trust: 'official' | 'unsigned' | 'unknownKey'
  keyId: string | null
  installedAt: number
  url: string | null
  /** 语言 → 名称（"*" 为未翻译的名称） */
  names?: Record<string, string>
  /** 已装版本要求的最低应用版本；高于当前版本时插件不会加载（应用降级后） */
  minAppVersion?: string | null
}

/** 简单比较 x.y.z 版本，用于判断是否有更新 */
function newer(a: string, b: string) {
  const pa = a.split(/[.-]/).map((n) => Number.parseInt(n, 10) || 0)
  const pb = b.split(/[.-]/).map((n) => Number.parseInt(n, 10) || 0)
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if ((pa[i] ?? 0) !== (pb[i] ?? 0)) return (pa[i] ?? 0) > (pb[i] ?? 0)
  }
  return false
}

type Pending = { kind: 'file'; path: string } | { kind: 'url'; entry: IndexEntry }

type DetailTarget = { kind: 'online'; entry: IndexEntry } | { kind: 'local'; info: InstallInfo }

/** 插件需要更高的应用版本时代替安装/打开按钮：有应用更新就打开更新弹窗，否则说明需要的版本尚未发布 */
function UpdateAppButton({ version, size }: { version: string; size?: 'sm' }) {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage('common')
  const status = useUpdate((s) => s.status)
  const onClick = async () => {
    const update = useUpdate.getState()
    if (['available', 'downloading', 'installing', 'ready'].includes(update.status)) {
      update.setDialogOpen(true)
      return
    }
    await update.check()
    const result = useUpdate.getState()
    if (result.status === 'latest') toast.info(t('market.online.noAppUpdate', { version }))
    else if (result.status === 'error') toast.error(errorMessage(result.error))
  }
  return (
    <Button size={size} variant="primary" loading={status === 'checking'} onClick={() => void onClick()}>
      <RefreshCw />
      {t('market.online.updateApp')}
    </Button>
  )
}

/** 在线插件图标的底色：与「全部工具」同一规则，accent 是什么颜色就显示什么颜色 */
function tileFor(manifest: Manifest | undefined, accent: string | null | undefined) {
  return accentTiles[manifest?.accent ?? accent ?? 'green'] ?? accentTiles.green
}

export function MarketOnline() {
  const { t, i18n } = useTranslation()
  const text = usePluginText()
  const errorMessage = useErrorMessage('common')
  const plugins = useApp((s) => s.plugins)
  const openTool = useApp((s) => s.openTool)
  const hiddenTools = usePrefs((s) => s.hiddenTools)
  const setHidden = usePrefs((s) => s.setToolHidden)
  const config = useToolConfig(plugins)
  const appVersion = useApp((s) => s.info?.version ?? '0.0.0')
  const [index, setIndex] = useState<IndexEntry[] | null>(null)
  const [indexError, setIndexError] = useState<AppError | null>(null)
  const [downloadStats, setDownloadStats] = useState<Map<string, PluginDownloadStats> | null>(null)
  const [installed, setInstalled] = useState<InstallInfo[]>([])
  /** 正在处理（排队、下载、安装或卸载）的插件 ID；文件安装用路径作为键 */
  const [busy, setBusy] = useState<ReadonlySet<string>>(() => new Set())
  const markBusy = (key: string, on: boolean) =>
    setBusy((current) => {
      const next = new Set(current)
      if (on) next.add(key)
      else next.delete(key)
      return next
    })
  const isBusy = (key: string) => busy.has(key)
  /** 文件安装的键是用户选择的路径，无法预知，所以用一个固定标记表示「正在从文件安装」 */
  const FILE_KEY = '\u0000file'
  const installingFile = busy.has(FILE_KEY)
  const [confirm, setConfirm] = useState<{ pending: Pending; error: AppError } | null>(null)
  const [detail, setDetail] = useState<DetailTarget | null>(null)

  const localized = (values: Record<string, string>) => values[i18n.language] ?? values['en-US'] ?? Object.values(values)[0] ?? ''
  /** 插件显示名：按当前语言取包内名称，没有时用 id */
  const displayName = (names: unknown, id: string) => {
    const map = names && typeof names === 'object' ? (names as Record<string, string>) : {}
    return map[i18n.language] ?? map['en-US'] ?? map['*'] ?? Object.values(map)[0] ?? id
  }
  /** 给安装错误补上 name 参数，提示里显示「名称（id）」 */
  const withName = (error: AppError): AppError => {
    const id = String(error.params?.id ?? '')
    return id ? { ...error, params: { ...error.params, name: displayName(error.params?.names, id) } } : error
  }

  /** 读取已安装列表与插件索引；只在回调中更新状态 */
  const load = useCallback(
    () =>
      Promise.all([
        invoke<InstallInfo[]>('plugin_installed').catch(() => [] as InstallInfo[]),
        invoke<{ plugins: IndexEntry[] }>('market_index').then(
          (data) => ({ plugins: data.plugins, error: null }),
          (error: AppError) => ({ plugins: [] as IndexEntry[], error }),
        ),
      ]).then(([list, result]) => {
        setInstalled(list)
        setIndex(result.plugins)
        setIndexError(result.error)
        setDownloadStats(null)
      }),
    [],
  )

  useEffect(() => {
    load()
  }, [load])

  useEffect(() => {
    if (!index) return
    let active = true
    void loadDownloadStats(index.map((entry) => entry.id)).then((stats) => {
      if (active) setDownloadStats(stats)
    })
    return () => { active = false }
  }, [index])

  const downloadCount = (value: string | undefined) => formatDownloadCount(value, i18n.language) ?? t(downloadStats === null ? 'market.online.statsLoading' : 'market.online.statsUnavailable')

  const refresh = () => {
    setIndex(null)
    void load()
  }

  const queue = useMemo(() => createInstallQueue(() => usePrefs.getState().maxConcurrentInstalls), [])
  const run = (pending: Pending, allowUnsigned: boolean) => {
    const key = pending.kind === 'url' ? pending.entry.id : FILE_KEY
    if (busy.has(key)) return Promise.resolve()
    markBusy(key, true)
    // 同一插件的下载/安装走队列，超过并发上限的任务等待
    return queue
      .run(async () => {
        const info =
          pending.kind === 'url'
            ? await invoke<InstallInfo>('plugin_install_url', { url: pending.entry.url, sha256: pending.entry.sha256, pluginId: pending.entry.id, version: pending.entry.version, size: pending.entry.size, allowUnsigned })
            : await invoke<InstallInfo>('plugin_install_file', { path: pending.path, allowUnsigned })
        toast.success(t('market.online.installed', { name: displayName(info.names, info.id), id: info.id, version: info.version }))
        setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
      })
      .catch((error: unknown) => {
        const appError = error as AppError
        if (appError.code === 'package.untrusted') setConfirm({ pending, error: withName(appError) })
        else toast.error(errorMessage(withName(appError)))
      })
      .finally(() => markBusy(key, false))
  }

  const installFile = async () => {
    const path = await host.dialog.openFile([{ name: t('market.online.packageFilter'), extensions: ['tfp'] }])
    if (path) await run({ kind: 'file', path }, false)
  }

  const uninstall = async (id: string) => {
    if (busy.has(id)) return
    markBusy(id, true)
    try {
      await invoke('plugin_uninstall', { pluginId: id })
      const info = installed.find((item) => item.id === id)
      toast.success(t('market.online.uninstalled', { name: displayName(info?.names, id), id }))
      setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
      setDetail(null)
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      markBusy(id, false)
    }
  }

  const local = installed.filter((info) => !index?.some((entry) => entry.id === info.id))
  /** 已装插件要求的应用版本高于当前版本时返回该版本（插件不会被加载） */
  const blockedBy = (info: InstallInfo | undefined) => (info?.minAppVersion && newer(info.minAppVersion, appVersion) ? info.minAppVersion : null)
  /** 在线条目因应用版本过低而不能安装、更新或使用时，返回需要的应用版本 */
  const requiredFor = (entry: IndexEntry, current: InstallInfo | undefined) => {
    const update = current && newer(entry.version, current.version)
    if (!current || update) return entry.minAppVersion && newer(entry.minAppVersion, appVersion) ? entry.minAppVersion : null
    return blockedBy(current)
  }

  // ---- 详情抽屉：在线条目与本地已装插件都映射为同一组字段 ----
  const detailManifest: Manifest | undefined = detail ? pluginById(plugins, detail.kind === 'online' ? detail.entry.id : detail.info.id) : undefined
  const detailHelp = detailManifest ? t('help', { ns: detailManifest.id, returnObjects: true, defaultValue: [] }) : []
  const detailHelpLines = Array.isArray(detailHelp) ? (detailHelp as string[]) : []

  const detailInfo: PluginDetailInfo | null = (() => {
    if (!detail) return null
    if (detail.kind === 'online') {
      const entry = detail.entry
      return {
        id: entry.id,
        name: detailManifest ? text(detailManifest, detailManifest.name) : localized(entry.names),
        description: detailManifest ? text(detailManifest, detailManifest.description) : localized(entry.descriptions),
        version: detailManifest?.version ?? entry.version,
        category: detailManifest?.category ?? entry.category,
        keywords: detailManifest?.keywords,
        permissions: detailManifest?.permissions,
        functionsCount: detailManifest ? Object.keys(detailManifest.functions).length : undefined,
        platforms: detailManifest?.platforms,
        sensitive: detailManifest?.sensitive,
        helpLines: detailHelpLines,
      }
    }
    const manifest = detailManifest
    return {
      id: detail.info.id,
      name: manifest ? text(manifest, manifest.name) : detail.info.id,
      description: manifest ? text(manifest, manifest.description) : '',
      version: detail.info.version,
      category: manifest?.category ?? 'other',
      keywords: manifest?.keywords,
      permissions: manifest?.permissions,
      functionsCount: manifest ? Object.keys(manifest.functions).length : undefined,
      platforms: manifest?.platforms,
      sensitive: manifest?.sensitive,
      helpLines: detailHelpLines,
    }
  })()

  const detailBlocked = detail?.kind === 'local' ? blockedBy(detail.info) : null

  const detailRows: PluginDetailRow[] = []
  if (detail?.kind === 'online') {
    const entry = detail.entry
    const stats = downloadStats?.get(entry.id)
    detailRows.push({ label: t('market.online.versionDownloads'), value: downloadCount(stats?.versions.find((item) => item.version === entry.version)?.downloads) })
    detailRows.push({ label: t('market.online.totalDownloadsLabel'), value: downloadCount(stats?.totalDownloads) })
    detailRows.push({ label: t('market.online.statsBasis'), value: t('market.online.statsHint') })
    detailRows.push({ label: t('market.online.packageSize'), value: t('market.online.size', { kb: Math.max(1, Math.round(entry.size / 1024)) }) })
    if (entry.minAppVersion) {
      detailRows.push({ label: t('market.online.minApp'), value: t('market.online.needsApp', { version: entry.minAppVersion }) })
    }
  } else if (detail) {
    detailRows.push({ label: t('market.online.installedAt'), value: new Date(detail.info.installedAt).toLocaleString() })
  }

  /** 已安装工具的图标操作；插件未加载或需要更新应用时不能打开 */
  const InstalledActions = ({ id, name, manifest, blocked }: { id: string; name: string; manifest: Manifest | undefined; blocked: string | null | undefined }) => (
    <ToolActions
      name={name}
      supported={!!manifest && config.supported(id)}
      onConfigure={() => manifest && config.configure(manifest)}
      onOpen={() => openTool(id)}
      openBlocked={blocked ? t('market.online.needsApp', { version: blocked }) : manifest ? null : t('market.actions.notLoaded')}
      hidden={hiddenTools.includes(id)}
      onToggleHidden={() => setHidden(id, !hiddenTools.includes(id))}
      removable
      busy={isBusy(id)}
      onUninstall={() => void uninstall(id)}
    />
  )

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="flex-1 text-xs text-fg-muted">{t('market.online.hint')}</p>
        <Button onClick={() => void refresh()}>
          <RefreshCw />
          {t('market.online.refresh')}
        </Button>
        <Button onClick={() => void installFile()} loading={installingFile}>
          <FileUp />
          {t('market.online.fromFile')}
        </Button>
      </div>

      {index === null ? (
        <div className="flex justify-center py-16 text-fg-muted">
          <Spinner />
        </div>
      ) : indexError ? (
        <Empty className="py-12" icon={<CloudDownload />} title={t('market.online.indexFailed')} description={errorMessage(indexError)} />
      ) : index.length === 0 ? (
        <Empty className="py-12" icon={<CloudDownload />} title={t('market.online.empty')} />
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-3">
          {index.map((entry) => {
            const current = installed.find((info) => info.id === entry.id)
            const manifest = pluginById(plugins, entry.id)
            const update = current && newer(entry.version, current.version)
            const required = requiredFor(entry, current)
            return (
              <div key={entry.id} className="flex flex-col gap-3 rounded-card border border-border bg-surface p-4 shadow-card">
                <div className="flex items-start gap-3">
                  {entry.icon ? (
                    <span aria-hidden className={cn(tileFor(manifest, entry.accent), 'inline-flex size-10 shrink-0 items-center justify-center rounded-[11px] text-on-tile shadow-tile [&>svg]:size-5')} dangerouslySetInnerHTML={{ __html: entry.icon }} />
                  ) : (
                    <span className={cn(tileFor(manifest, entry.accent), 'inline-flex size-10 shrink-0 items-center justify-center rounded-[11px] text-on-tile shadow-tile')}>
                      <PackageCheck className="size-5" />
                    </span>
                  )}
                  <button
                    type="button"
                    onClick={() => setDetail({ kind: 'online', entry })}
                    className="min-w-0 flex-1 rounded-control text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  >
                    <div className="flex flex-wrap items-center gap-1.5">
                      <p className="truncate text-sm font-semibold text-fg">{manifest ? text(manifest, manifest.name) : localized(entry.names)}</p>
                      {entry.signed && (
                        <Badge variant="success">
                          <ShieldCheck className="size-3" />
                          {t('market.online.official')}
                        </Badge>
                      )}
                    </div>
                    <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-fg-muted">{localized(entry.descriptions)}</p>
                  </button>
                </div>
                <p className="text-[11px] text-fg-subtle" title={t('market.online.statsHint')}>
                  {t('market.online.totalDownloads', { value: downloadCount(downloadStats?.get(entry.id)?.totalDownloads) })}
                </p>
                {required && <p className="text-[11px] text-warning">{t('market.online.needsApp', { version: required })}</p>}
                <div className="mt-auto flex items-center gap-2 text-[11px] text-fg-subtle">
                  <span>{t('market.version', { version: entry.version })}</span>
                  <span>·</span>
                  <span>{t(`category.${entry.category}`, { defaultValue: entry.category })}</span>
                  <div className="ml-auto flex items-center gap-1.5">
                    {current && (
                      <InstalledActions id={entry.id} name={manifest ? text(manifest, manifest.name) : localized(entry.names)} manifest={manifest} blocked={required} />
                    )}
                    {required && <UpdateAppButton size="sm" version={required} />}
                    {(!current || update) && !required && (
                      <Button size="sm" variant="primary" loading={isBusy(entry.id)} disabled={isBusy(entry.id)} onClick={() => void run({ kind: 'url', entry }, false)}>
                        <CloudDownload />
                        {update ? t('market.online.update', { version: entry.version }) : t('market.online.install')}
                      </Button>
                    )}
                  </div>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {local.length > 0 && (
        <section className="space-y-2">
          <h3 className="text-xs font-semibold text-fg-muted">{t('market.online.local')}</h3>
          <ul className="space-y-2">
            {local.map((info) => {
              const manifest = pluginById(plugins, info.id)
              const required = blockedBy(info)
              return (
                <li key={info.id} className="flex items-center gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
                  <button
                    type="button"
                    onClick={() => setDetail({ kind: 'local', info })}
                    className="min-w-0 flex-1 rounded-control text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  >
                    <p className="truncate text-[13px] font-medium text-fg">{manifest ? text(manifest, manifest.name) : info.id}</p>
                    <p className="text-[11px] text-fg-subtle">
                      {t('market.version', { version: info.version })} · {t(`market.online.trust.${info.trust}`)}
                    </p>
                    {required && <p className="text-[11px] text-warning">{t('market.online.needsApp', { version: required })}</p>}
                  </button>
                  {required && <UpdateAppButton size="sm" version={required} />}
                  <InstalledActions id={info.id} name={manifest ? text(manifest, manifest.name) : info.id} manifest={manifest} blocked={required} />
                </li>
              )
            })}
          </ul>
        </section>
      )}

      <PluginDetail
        info={detailInfo}
        icon={
          detailManifest ? (
            <ToolTile manifest={detailManifest} size="lg" />
          ) : detail?.kind === 'online' && detail.entry.icon ? (
            <span aria-hidden className={cn(tileFor(detailManifest, detail.entry.accent), 'inline-flex size-16 shrink-0 items-center justify-center rounded-tile text-on-tile shadow-tile [&>svg]:size-8')} dangerouslySetInnerHTML={{ __html: detail.entry.icon }} />
          ) : (
            <span className={cn(tileFor(detailManifest, detail?.kind === 'online' ? detail.entry.accent : undefined), 'inline-flex size-16 shrink-0 items-center justify-center rounded-tile text-on-tile shadow-tile')}>
              <PackageCheck className="size-8" />
            </span>
          )}
        originBadge={
          detail?.kind === 'online' ? (
            detail.entry.signed ? (
              <Badge variant="success">
                <ShieldCheck className="size-3" />
                {t('market.online.official')}
              </Badge>
            ) : undefined
          ) : detail ? (
            <Badge variant={detail.info.trust === 'official' ? 'success' : 'warning'}>{t(`market.online.trust.${detail.info.trust}`)}</Badge>
          ) : undefined
        }
        actions={
          detail?.kind === 'online' ? (
            <>
              {(() => {
                const entry = detail.entry
                const current = installed.find((info) => info.id === entry.id)
                const update = current && newer(entry.version, current.version)
                const required = requiredFor(entry, current)
                return (
                  <>
                    {required && <UpdateAppButton version={required} />}
                    {(!current || update) && !required && (
                      <Button variant="primary" loading={isBusy(entry.id)} disabled={isBusy(entry.id)} onClick={() => void run({ kind: 'url', entry }, false)}>
                        <CloudDownload />
                        {update ? t('market.online.update', { version: entry.version }) : t('market.online.install')}
                      </Button>
                    )}
                    {current && detailManifest && !update && !required && (
                      <Button variant="ghost" onClick={() => { setDetail(null); openTool(entry.id) }}>
                        <SquareArrowOutUpRight />
                        {t('market.open')}
                      </Button>
                    )}
                    {current && (
                      <Button variant="ghost" disabled={isBusy(entry.id)} onClick={() => void uninstall(entry.id)}>
                        <Trash2 />
                        {t('market.online.uninstall')}
                      </Button>
                    )}
                    {required && <span className="text-xs text-warning">{t('market.online.needsApp', { version: required })}</span>}
                  </>
                )
              })()}
            </>
          ) : detail ? (
            <>
              {detailBlocked && (
                <>
                  <UpdateAppButton version={detailBlocked} />
                  <span className="text-xs text-warning">{t('market.online.needsApp', { version: detailBlocked })}</span>
                </>
              )}
              {detailManifest && !detailBlocked && (
                <Button variant="primary" onClick={() => { setDetail(null); openTool(detail.info.id) }}>
                  <SquareArrowOutUpRight />
                  {t('market.open')}
                </Button>
              )}
              <Button variant="ghost" disabled={isBusy(detail.info.id)} onClick={() => void uninstall(detail.info.id)}>
                <Trash2 />
                {t('market.online.uninstall')}
              </Button>
            </>
          ) : undefined
        }
        extraRows={detailRows}
        onClose={() => setDetail(null)}
      />

      {config.dialog}
      <Modal
        open={confirm !== null}
        onOpenChange={(open) => !open && setConfirm(null)}
        title={t('market.online.confirmTitle')}
        closeLabel={t('common:close')}
        footer={
          <>
            <Button onClick={() => setConfirm(null)}>{t('common:cancel')}</Button>
            <Button
              variant="danger"
              onClick={() => {
                const pending = confirm?.pending
                setConfirm(null)
                if (pending) void run(pending, true)
              }}
            >
              <ShieldAlert />
              {t('market.online.confirmInstall')}
            </Button>
          </>
        }
      >
        {confirm && (
          <div className="space-y-2 px-5 pb-5 text-[13px] text-fg">
            <p>
              {t(`market.online.untrusted.${String(confirm.error.params?.trust ?? 'unsigned')}`, {
                id: String(confirm.error.params?.id ?? ''),
                name: String(confirm.error.params?.name ?? confirm.error.params?.id ?? ''),
                version: String(confirm.error.params?.version ?? ''),
                keyId: String(confirm.error.params?.keyId ?? ''),
              })}
            </p>
            <p className="text-xs text-fg-muted">{t('market.online.confirmHint')}</p>
          </div>
        )}
      </Modal>
    </div>
  )
}
