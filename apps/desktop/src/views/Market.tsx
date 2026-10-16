import { useMemo, useState } from 'react'
import { Badge, Button, Drawer, Empty, Input, SegmentedControl, Select, Switch, Tabs, cn } from '@toolforge/ui'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { CloudDownload, LayoutGrid, PackageSearch, Search, SquareArrowOutUpRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { CATEGORIES } from '../lib/categories'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { useApp } from '../stores/app'
import { toolStatus, usePrefs, type ToolStatus } from '../stores/prefs'
import { MarketOnline } from './MarketOnline'
import { Page } from './Page'

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

function Detail({ manifest, onClose }: { manifest: Manifest | null; onClose: () => void }) {
  const { t } = useTranslation()
  const text = usePluginText()
  const openTool = useApp((s) => s.openTool)
  const hidden = usePrefs((s) => s.hiddenTools)
  const setHidden = usePrefs((s) => s.setToolHidden)
  const help = manifest ? t('help', { ns: manifest.id, returnObjects: true, defaultValue: [] }) : []
  const lines = Array.isArray(help) ? (help as string[]) : []

  return (
    <Drawer open={manifest !== null} onOpenChange={(open) => !open && onClose()} title={manifest ? text(manifest, manifest.name) : ''} closeLabel={t('common:close')}>
      {manifest && (
        <div className="space-y-5 p-4">
          <div className="flex items-start gap-3">
            <ToolTile manifest={manifest} size="lg" />
            <div className="min-w-0 space-y-1.5">
              <p className="text-[13px] leading-relaxed text-fg-muted">{text(manifest, manifest.description)}</p>
              <div className="flex flex-wrap gap-1.5">
                <Badge>{t('market.version', { version: manifest.version })}</Badge>
                <Badge>{t(`category.${manifest.category}`, { defaultValue: manifest.category })}</Badge>
                <Badge variant="success">{t('market.builtin')}</Badge>
                {manifest.sensitive && <Badge variant="warning">{t('market.sensitive')}</Badge>}
              </div>
            </div>
          </div>
          <div className="flex gap-2">
            <Button
              variant="primary"
              onClick={() => {
                onClose()
                openTool(manifest.id)
              }}
            >
              <SquareArrowOutUpRight />
              {t('market.open')}
            </Button>
            <label className="ml-auto flex items-center gap-2 text-[13px] text-fg">
              <Switch size="sm" checked={!hidden.includes(manifest.id)} onCheckedChange={(on) => setHidden(manifest.id, !on)} aria-label={t('market.visible')} />
              {t('market.visible')}
            </label>
          </div>
          {lines.length > 0 && (
            <section className="space-y-2">
              <h3 className="text-xs font-semibold text-fg-muted">{t('market.features')}</h3>
              <ul className="list-disc space-y-1.5 pl-4 text-[13px] leading-relaxed text-fg">
                {lines.map((line, index) => (
                  <li key={index}>{line}</li>
                ))}
              </ul>
            </section>
          )}
          <section className="space-y-2 text-xs">
            <h3 className="font-semibold text-fg-muted">{t('market.details')}</h3>
            <dl className="grid grid-cols-[96px_minmax(0,1fr)] gap-x-3 gap-y-1.5">
              <dt className="text-fg-subtle">{t('market.id')}</dt>
              <dd className="font-mono break-all text-fg" data-selectable>
                {manifest.id}
              </dd>
              <dt className="text-fg-subtle">{t('market.functions')}</dt>
              <dd className="text-fg">{Object.keys(manifest.functions).length}</dd>
              <dt className="text-fg-subtle">{t('market.permissions')}</dt>
              <dd className="flex flex-wrap gap-1">
                {(manifest.permissions ?? []).length === 0 ? (
                  <span className="text-fg-muted">{t('market.none')}</span>
                ) : (
                  manifest.permissions?.map((permission) => (
                    <code key={permission} className="rounded-sm bg-surface-2 px-1.5 py-0.5 font-mono text-fg">
                      {permission}
                    </code>
                  ))
                )}
              </dd>
              <dt className="text-fg-subtle">{t('market.platforms')}</dt>
              <dd className="text-fg">{manifest.platforms?.length ? manifest.platforms.join(' / ') : t('market.allPlatforms')}</dd>
              {manifest.keywords.length > 0 && (
                <>
                  <dt className="text-fg-subtle">{t('market.keywords')}</dt>
                  <dd className="text-fg-muted">{manifest.keywords.join(' · ')}</dd>
                </>
              )}
            </dl>
          </section>
        </div>
      )}
    </Drawer>
  )
}

export function Market() {
  const { t } = useTranslation()
  const text = usePluginText()
  const plugins = useApp((s) => s.plugins)
  const hidden = usePrefs((s) => s.hiddenTools)
  const seen = usePrefs((s) => s.seenVersions)
  const setHidden = usePrefs((s) => s.setToolHidden)
  const markSeen = usePrefs((s) => s.markToolsSeen)
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('all')
  const [filter, setFilter] = useState<Filter>('all')
  const [selected, setSelected] = useState<Manifest | null>(null)
  const [tab, setTab] = useState<'tools' | 'online'>('tools')

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
                  <label className="ml-auto flex items-center gap-1.5 text-xs text-fg-muted">
                    {t('market.visible')}
                    <Switch size="sm" checked={!isHidden} onCheckedChange={(on) => setHidden(manifest.id, !on)} aria-label={t('market.visibleFor', { name: text(manifest, manifest.name) })} />
                  </label>
                </div>
              </div>
            )
          })}
        </div>
      )}
        </>
      )}
      <Detail manifest={selected} onClose={() => setSelected(null)} />
    </Page>
  )
}
