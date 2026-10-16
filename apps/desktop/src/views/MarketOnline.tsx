import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Badge, Button, Empty, Modal, Spinner, toast } from '@toolforge/ui'
import { host, useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { CloudDownload, FileUp, PackageCheck, RefreshCw, ShieldAlert, ShieldCheck, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePluginText } from '../plugins/usePluginText'
import { pluginById, useApp } from '../stores/app'

interface IndexEntry {
  id: string
  version: string
  category: string
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

  const localized = (values: Record<string, string>) => values[i18n.language] ?? values['en-US'] ?? Object.values(values)[0] ?? ''

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
      toast.success(t('market.online.installed', { version: info.version }))
      setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
    } catch (error) {
      const appError = error as AppError
      if (appError.code === 'package.untrusted') setConfirm({ pending, error: appError })
      else toast.error(errorMessage(appError))
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
      toast.success(t('market.online.uninstalled'))
      setInstalled(await invoke<InstallInfo[]>('plugin_installed'))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(null)
    }
  }

  const local = installed.filter((info) => !index?.some((entry) => entry.id === info.id))

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
                    <span aria-hidden className="tile-green inline-flex size-10 shrink-0 items-center justify-center rounded-[11px] text-on-tile shadow-tile [&>svg]:size-5" dangerouslySetInnerHTML={{ __html: entry.icon }} />
                  ) : (
                    <span className="tile-green inline-flex size-10 shrink-0 items-center justify-center rounded-[11px] text-on-tile shadow-tile">
                      <PackageCheck className="size-5" />
                    </span>
                  )}
                  <div className="min-w-0 flex-1">
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
                  </div>
                </div>
                <div className="mt-auto flex items-center gap-2 text-[11px] text-fg-subtle">
                  <span>{t('market.version', { version: entry.version })}</span>
                  <span>·</span>
                  <span>{t('market.online.size', { kb: Math.max(1, Math.round(entry.size / 1024)) })}</span>
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
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[13px] font-medium text-fg">{manifest ? text(manifest, manifest.name) : info.id}</p>
                    <p className="text-[11px] text-fg-subtle">
                      {t('market.version', { version: info.version })} · {t(`market.online.trust.${info.trust}`)}
                    </p>
                  </div>
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
