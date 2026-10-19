import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge, Button, Empty, Modal, Spinner, cn, toast } from '@toolforge/ui'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { host, useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { CloudDownload, FileUp, PackageCheck, RefreshCw, ShieldAlert, ShieldCheck, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { ToolTile, accentTiles } from '../plugins/ToolIcon'
import { pluginById, useApp } from '../stores/app'
import { PluginDetail, type PluginDetailInfo, type PluginDetailRow } from './PluginDetail'

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
  const appVersion = useApp((s) => s.info?.version ?? '0.0.0')
  const [index, setIndex] = useState<IndexEntry[] | null>(null)
  const [indexError, setIndexError] = useState<AppError | null>(null)
  const [installed, setInstalled] = useState<InstallInfo[]>([])
  const [busy, setBusy] = useState<string | null>(null)
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
      }),
    [],
  )

  useEffect(() => {
    load()
  }, [load])

  const refresh = () => {
    setIndex(null)
    void load()
  }

  const run = async (pending: Pending, allowUnsigned: boolean) => {
    const id = pending.kind === 'url' ? pending.entry.id : pending.path
    setBusy(id)
    try {
      const info =
        pending.kind === 'url'
          ? await invoke<InstallInfo>('plugin_install_url', { url: pending.entry.url, sha256: pending.entry.sha256, allowUnsigned })
          : await invoke<InstallInfo>('plugin_install_file', { path: pending.path, allowUnsigned })
      toast.success(t('market.online.installed', { name: displayName(info.names, info.id), id: info.id, version: info.version }))
      setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
    } catch (error) {
      const appError = error as AppError
      if (appError.code === 'package.untrusted') setConfirm({ pending, error: withName(appError) })
      else toast.error(errorMessage(withName(appError)))
    } finally {
      setBusy(null)
    }
  }

  const installFile = async () => {
    const path = await host.dialog.openFile([{ name: t('market.online.packageFilter'), extensions: ['tfp'] }])
    if (path) await run({ kind: 'file', path }, false)
  }

  const uninstall = async (id: string) => {
    setBusy(id)
    try {
      await invoke('plugin_uninstall', { pluginId: id })
      const info = installed.find((item) => item.id === id)
      toast.success(t('market.online.uninstalled', { name: displayName(info?.names, id), id }))
      setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
      setDetail(null)
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(null)
    }
  }

  const local = installed.filter((info) => !index?.some((entry) => entry.id === info.id))

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

  const detailRows: PluginDetailRow[] = []
  if (detail?.kind === 'online') {
    const entry = detail.entry
    detailRows.push({ label: t('market.online.packageSize'), value: t('market.online.size', { kb: Math.max(1, Math.round(entry.size / 1024)) }) })
    if (entry.minAppVersion) {
      detailRows.push({ label: t('market.online.minApp'), value: t('market.online.needsApp', { version: entry.minAppVersion }) })
    }
  } else if (detail) {
    detailRows.push({ label: t('market.online.installedAt'), value: new Date(detail.info.installedAt).toLocaleString() })
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <p className="flex-1 text-xs text-fg-muted">{t('market.online.hint')}</p>
        <Button onClick={() => void refresh()}>
          <RefreshCw />
          {t('market.online.refresh')}
        </Button>
        <Button onClick={() => void installFile()} loading={busy !== null && !index?.some((e) => e.id === busy) && !installed.some((i) => i.id === busy)}>
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
            const tooOld = !!entry.minAppVersion && newer(entry.minAppVersion, appVersion)
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
                <div className="mt-auto flex items-center gap-2 text-[11px] text-fg-subtle">
                  <span>{t('market.version', { version: entry.version })}</span>
                  <span>·</span>
                  <span>{t(`category.${entry.category}`, { defaultValue: entry.category })}</span>
                  <div className="ml-auto flex items-center gap-1.5">
                    {current && !update && manifest && (
                      <Button size="sm" variant="ghost" onClick={() => openTool(entry.id)}>
                        {t('market.open')}
                      </Button>
                    )}
                    {current && (
                      <Button size="icon-sm" variant="ghost" aria-label={t('market.online.uninstall')} disabled={busy !== null} onClick={() => void uninstall(entry.id)}>
                        <Trash2 />
                      </Button>
                    )}
                    {(!current || update) && tooOld && <span className="text-warning">{t('market.online.needsApp', { version: entry.minAppVersion })}</span>}
                    {(!current || update) && !tooOld && (
                      <Button size="sm" variant="primary" loading={busy === entry.id} disabled={busy !== null} onClick={() => void run({ kind: 'url', entry }, false)}>
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
                  </button>
                  {manifest && (
                    <Button size="sm" variant="ghost" onClick={() => openTool(info.id)}>
                      {t('market.open')}
                    </Button>
                  )}
                  <Button size="icon-sm" variant="ghost" aria-label={t('market.online.uninstall')} disabled={busy !== null} onClick={() => void uninstall(info.id)}>
                    <Trash2 />
                  </Button>
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
                const tooOld = !!entry.minAppVersion && newer(entry.minAppVersion, appVersion)
                return (
                  <>
                    {(!current || update) && !tooOld && (
                      <Button variant="primary" loading={busy === entry.id} disabled={busy !== null} onClick={() => void run({ kind: 'url', entry }, false)}>
                        <CloudDownload />
                        {update ? t('market.online.update', { version: entry.version }) : t('market.online.install')}
                      </Button>
                    )}
                    {current && detailManifest && !update && (
                      <Button variant="ghost" onClick={() => { setDetail(null); openTool(entry.id) }}>
                        <SquareArrowOutUpRight />
                        {t('market.open')}
                      </Button>
                    )}
                    {current && (
                      <Button variant="ghost" disabled={busy !== null} onClick={() => void uninstall(entry.id)}>
                        <Trash2 />
                        {t('market.online.uninstall')}
                      </Button>
                    )}
                    {tooOld && <span className="text-xs text-warning">{t('market.online.needsApp', { version: entry.minAppVersion })}</span>}
                  </>
                )
              })()}
            </>
          ) : detail ? (
            <>
              {detailManifest && (
                <Button variant="primary" onClick={() => { setDetail(null); openTool(detail.info.id) }}>
                  <SquareArrowOutUpRight />
                  {t('market.open')}
                </Button>
              )}
              <Button variant="ghost" disabled={busy !== null} onClick={() => void uninstall(detail.info.id)}>
                <Trash2 />
                {t('market.online.uninstall')}
              </Button>
            </>
          ) : undefined
        }
        extraRows={detailRows}
        onClose={() => setDetail(null)}
      />

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
          <div className="space-y-2 text-[13px] text-fg">
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
