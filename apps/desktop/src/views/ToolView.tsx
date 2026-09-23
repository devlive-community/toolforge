import { Component, Suspense, useState, type ErrorInfo, type ReactNode } from 'react'
import { Button, DropdownMenu, Empty, Modal, Spinner, cn } from '@toolforge/ui'
import { PluginProvider, type Manifest } from '@toolforge/plugin-ui-sdk'
import { CircleHelp, Ellipsis, Info, PackageX, RotateCw, Star, TriangleAlert } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { pluginComponent } from '../plugins/modules'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { pluginById, useApp } from '../stores/app'

class PluginBoundary extends Component<{ fallback: (error: Error) => ReactNode; children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null }

  static getDerivedStateFromError(error: Error) {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[plugin]', error, info.componentStack)
  }

  render() {
    return this.state.error ? this.props.fallback(this.state.error) : this.props.children
  }
}

function ToolHeader({ manifest, onReload }: { manifest: Manifest; onReload: () => void }) {
  const { t } = useTranslation()
  const text = usePluginText()
  const favored = useApp((s) => s.favorites.includes(manifest.id))
  const toggleFavorite = useApp((s) => s.toggleFavorite)
  const [helpOpen, setHelpOpen] = useState(false)
  const [aboutOpen, setAboutOpen] = useState(false)
  const help = t('help', { ns: manifest.id, returnObjects: true, defaultValue: [] }) as unknown
  const helpLines = Array.isArray(help) ? (help as string[]) : []

  return (
    <header className="flex items-center gap-4">
      <ToolTile manifest={manifest} size="lg" />
      <div className="min-w-0 flex-1">
        <h1 className="truncate text-[26px] leading-tight font-bold tracking-tight text-fg">
          {text(manifest, manifest.name)}
        </h1>
        <p className="mt-1 truncate text-[13px] text-fg-muted">{text(manifest, manifest.description)}</p>
      </div>
      <div className="flex items-center gap-2">
        <Button onClick={() => toggleFavorite(manifest.id)} aria-pressed={favored}>
          <Star className={cn(favored && 'fill-warning text-warning')} />
          {t(favored ? 'tool.unfavorite' : 'tool.favorite')}
        </Button>
        {helpLines.length > 0 && (
          <Button onClick={() => setHelpOpen(true)}>
            <CircleHelp />
            {t('tool.help')}
          </Button>
        )}
        <DropdownMenu
          trigger={
            <Button size="icon-md" aria-label={t('tool.more')}>
              <Ellipsis />
            </Button>
          }
          items={[
            { key: 'reload', label: t('tool.reload'), icon: <RotateCw />, onSelect: onReload },
            { key: 'about', label: t('tool.about'), icon: <Info />, onSelect: () => setAboutOpen(true) },
          ]}
        />
      </div>

      <Modal open={helpOpen} onOpenChange={setHelpOpen} title={t('tool.help')} closeLabel={t('common:close')}>
        <ul className="space-y-2 px-5 pb-5 text-[13px] leading-relaxed text-fg-muted" data-selectable>
          {helpLines.map((line) => (
            <li key={line} className="flex gap-2">
              <span className="mt-2 size-1 shrink-0 rounded-full bg-primary" />
              {line}
            </li>
          ))}
        </ul>
      </Modal>
      <Modal
        open={aboutOpen}
        onOpenChange={setAboutOpen}
        title={text(manifest, manifest.name)}
        size="sm"
        closeLabel={t('common:close')}
      >
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 px-5 pb-5 text-[13px]" data-selectable>
          <dt className="text-fg-muted">{t('tool.pluginId')}</dt>
          <dd className="font-mono text-xs text-fg">{manifest.id}</dd>
          <dt className="text-fg-muted">{t('tool.version')}</dt>
          <dd className="text-fg">{manifest.version}</dd>
          <dt className="text-fg-muted">{t('tool.category')}</dt>
          <dd className="text-fg">{t(`category.${manifest.category}`)}</dd>
        </dl>
      </Modal>
    </header>
  )
}

export function ToolView({ pluginId }: { pluginId: string }) {
  const { t } = useTranslation()
  const manifest = useApp((s) => pluginById(s.plugins, pluginId))
  const [reloadKey, setReloadKey] = useState(0)

  if (!manifest) {
    return <Empty icon={<PackageX />} title={t('tool.notFound')} />
  }
  const PluginUI = pluginComponent(manifest.id)

  return (
    <div className="flex h-full min-h-0 animate-fade-in flex-col gap-4 px-6 pt-5 pb-5">
      <ToolHeader manifest={manifest} onReload={() => setReloadKey((k) => k + 1)} />
      <div className="min-h-0 flex-1">
        {PluginUI ? (
          <PluginBoundary
            key={`${manifest.id}-${reloadKey}`}
            fallback={(error) => (
              <Empty
                icon={<TriangleAlert />}
                title={t('tool.crashed')}
                description={error.message}
                action={
                  <Button onClick={() => setReloadKey((k) => k + 1)}>
                    <RotateCw />
                    {t('tool.reload')}
                  </Button>
                }
              />
            )}
          >
            <Suspense
              fallback={
                <div className="flex h-full items-center justify-center text-fg-muted">
                  <Spinner />
                </div>
              }
            >
              <PluginProvider manifest={manifest}>
                <PluginUI />
              </PluginProvider>
            </Suspense>
          </PluginBoundary>
        ) : (
          <Empty icon={<PackageX />} title={t('tool.noUi')} />
        )}
      </div>
    </div>
  )
}
