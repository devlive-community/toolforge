import { useEffect, useState, type ReactNode } from 'react'
import { Badge, Button, Empty, Input, LogViewer, NumberInput, Panel, Switch, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ExternalLink, FolderOpen, Globe, Link2, Play, ScrollText, Square, TriangleAlert, Wand2 } from 'lucide-react'
import { useServer } from './useServer'
import { DEFAULT_SETTINGS, type Addresses, type Settings } from './types'

function Toggle({ label, hint, checked, onChange, disabled }: { label: string; hint: string; checked: boolean; onChange: (value: boolean) => void; disabled: boolean }) {
  return (
    <Tooltip content={hint}>
      <label className={cn('flex items-center gap-2 text-[13px] text-fg', disabled && 'opacity-60')}>
        <Switch size="sm" checked={checked} onCheckedChange={onChange} disabled={disabled} aria-label={label} />
        {label}
      </label>
    </Tooltip>
  )
}

function Field({ label, children, className }: { label: string; children: ReactNode; className?: string }) {
  return (
    <label className={cn('flex flex-col gap-1.5', className)}>
      <span className="text-xs font-medium text-fg-muted">{label}</span>
      {children}
    </label>
  )
}

function LocalServer() {
  const { t, call, errorMessage } = usePlugin()
  const [settings, setSettings, loaded] = usePluginState<Settings>('settings', DEFAULT_SETTINGS)
  const server = useServer()
  const [selectedQr, setSelectedQr] = useState(0)
  const update = (patch: Partial<Settings>) => setSettings({ ...settings, ...patch })
  const locked = server.running

  const info = useDebouncedCall<Addresses>(loaded ? 'addresses' : null, { port: settings.port, lan: settings.lan }, [settings.port, settings.lan, loaded, server.running])
  const addresses = info.result?.addresses ?? []
  const lanAddresses = addresses.filter((a) => a.kind === 'lan')
  const qr = lanAddresses[Math.min(selectedQr, lanAddresses.length - 1)]
  const portBusy = !server.running && info.result !== null && !info.result.available && !info.pending

  useEffect(() => {
    if (server.error && server.error.code !== 'task.cancelled') toast.error(errorMessage(server.error))
  }, [server.error, errorMessage])

  const pickFolder = async () => {
    const picked = await host.dialog.openDirectory()
    if (picked) update({ root: picked })
  }

  const findFreePort = async () => {
    try {
      update({ port: await call<number>('free_port', { start: settings.port, lan: settings.lan }) })
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex flex-col gap-3 rounded-card border border-border bg-surface p-3 shadow-card">
        <div className="flex flex-wrap items-end gap-3">
          <Field label={t('settings.root')} className="min-w-0 flex-1">
            <Input
              value={settings.root}
              onChange={(event) => update({ root: event.target.value })}
              placeholder={t('settings.rootPlaceholder')}
              disabled={locked}
              className="font-mono"
              spellCheck={false}
              trailing={
                <Button size="sm" variant="ghost" onClick={pickFolder} disabled={locked}>
                  <FolderOpen />
                  {t('settings.choose')}
                </Button>
              }
            />
          </Field>
          <Field label={t('settings.port')}>
            <NumberInput
              value={settings.port}
              onValueChange={(port) => update({ port })}
              min={1}
              max={65535}
              disabled={locked}
              className="w-32"
              aria-label={t('settings.port')}
            />
          </Field>
          {server.running ? (
            <Button variant="danger" onClick={server.stop}>
              <Square />
              {t('actions.stop')}
            </Button>
          ) : (
            <Button variant="primary" onClick={() => server.start(settings)} disabled={!settings.root.trim()}>
              <Play />
              {t('actions.start')}
            </Button>
          )}
        </div>
        <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
          <Toggle label={t('settings.lan')} hint={t('settings.lanHint')} checked={settings.lan} onChange={(lan) => update({ lan })} disabled={locked} />
          <Toggle label={t('settings.listing')} hint={t('settings.listingHint')} checked={settings.listing} onChange={(listing) => update({ listing })} disabled={locked} />
          <Toggle label={t('settings.spa')} hint={t('settings.spaHint')} checked={settings.spa} onChange={(spa) => update({ spa })} disabled={locked} />
          <Toggle label={t('settings.cors')} hint={t('settings.corsHint')} checked={settings.cors} onChange={(cors) => update({ cors })} disabled={locked} />
          <Toggle
            label={t('settings.hideDotfiles')}
            hint={t('settings.hideDotfilesHint')}
            checked={settings.hideDotfiles}
            onChange={(hideDotfiles) => update({ hideDotfiles })}
            disabled={locked}
          />
          {portBusy && (
            <span className="ml-auto flex items-center gap-2 text-xs text-warning">
              <TriangleAlert className="size-3.5" />
              {t('settings.portBusy', { port: settings.port })}
              <Button size="sm" onClick={findFreePort}>
                <Wand2 />
                {t('settings.freePort')}
              </Button>
            </span>
          )}
        </div>
      </section>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,2fr)_3fr] gap-3">
        <Panel
          icon={<Link2 />}
          title={t('addresses.title')}
          extra={<Badge variant={server.running ? 'success' : 'neutral'}>{t(server.running ? 'status.running' : 'status.stopped')}</Badge>}
          bodyClassName="flex flex-col gap-3 overflow-auto p-3"
        >
          {server.reattached && <p className="rounded-control bg-info-soft px-2.5 py-2 text-xs text-info">{t('addresses.reattached')}</p>}
          <ul className="flex flex-col gap-1.5">
            {addresses.map((address) => (
              <li key={address.url} className="flex items-center gap-2 rounded-control border border-border bg-surface-2 px-2.5 py-1.5">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-mono text-[13px] text-fg" data-selectable>
                    {address.url}
                  </p>
                  <p className="truncate text-[11px] text-fg-subtle">
                    {address.kind === 'lan' ? t('addresses.lan', { name: address.interface }) : t('addresses.local')}
                  </p>
                </div>
                <CopyButton text={address.url} size="icon-sm" />
                <Tooltip content={t('addresses.open')}>
                  <Button size="icon-sm" variant="ghost" onClick={() => host.openUrl(address.url)} disabled={!server.running} aria-label={t('addresses.open')}>
                    <ExternalLink />
                  </Button>
                </Tooltip>
              </li>
            ))}
          </ul>
          {settings.lan && (
            <>
              <p className="flex items-start gap-1.5 rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">
                <TriangleAlert className="mt-px size-3.5 shrink-0" />
                {t('addresses.lanWarning')}
              </p>
              {qr ? (
                <div className="flex flex-col items-center gap-2">
                  {qr.qr && <img src={qr.qr} alt={qr.url} className="size-44 rounded-control border border-border" />}
                  {lanAddresses.length > 1 && (
                    <div className="flex flex-wrap justify-center gap-1">
                      {lanAddresses.map((address, index) => (
                        <Button key={address.url} size="sm" variant={address === qr ? 'primary' : 'ghost'} onClick={() => setSelectedQr(index)}>
                          {address.interface}
                        </Button>
                      ))}
                    </div>
                  )}
                  <p className="text-center text-xs text-fg-muted">{t('addresses.scan')}</p>
                </div>
              ) : (
                <p className="text-xs text-fg-muted">{t('addresses.noLan')}</p>
              )}
            </>
          )}
        </Panel>

        <Panel icon={<ScrollText />} title={t('requests.title')} bodyClassName="p-0">
          {server.logs.length > 0 || server.running ? (
            <LogViewer entries={server.logs} emptyText={t('requests.waiting')} jumpLabel={t('requests.jumpLatest')} />
          ) : (
            <Empty icon={<Globe />} title={t('requests.placeholder')} description={t('requests.placeholderHint')} />
          )}
        </Panel>
      </div>
    </div>
  )
}

export default LocalServer
