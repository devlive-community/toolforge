import { cn } from '@toolforge/ui'
import { ChevronDown, Clock3, House, Star } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { CATEGORIES } from '../lib/categories'
import { ToolGlyph } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { useApp, type Route } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { Logo } from './Logo'

function NavItem({
  icon,
  label,
  active,
  onClick,
  indent,
}: {
  icon: ReactNode
  label: string
  active?: boolean
  onClick: () => void
  indent?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? 'page' : undefined}
      className={cn(
        'flex h-9 w-full items-center gap-3 rounded-control text-[13px] outline-none transition-colors',
        'focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-4 [&_svg]:shrink-0',
        indent ? 'pr-2 pl-5' : 'px-3',
        active ? 'bg-primary font-medium text-fg-on-primary shadow-card' : 'text-fg hover:bg-hover',
      )}
    >
      <span className={cn('flex', active ? 'text-fg-on-primary' : 'text-fg-muted')}>{icon}</span>
      <span className="truncate">{label}</span>
    </button>
  )
}

export function Sidebar() {
  const { t } = useTranslation()
  const text = usePluginText()
  const plugins = useApp((s) => s.plugins)
  const route = useApp((s) => s.route)
  const info = useApp((s) => s.info)
  const navigate = useApp((s) => s.navigate)
  const openTool = useApp((s) => s.openTool)
  const collapsed = usePrefs((s) => s.collapsed)
  const toggleCategory = usePrefs((s) => s.toggleCategory)

  const is = (view: Route['view']) => route.view === view
  const activeTool = route.view === 'tool' ? route.pluginId : null

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-border bg-sidebar">
      <nav className="flex-1 space-y-1 overflow-y-auto px-3 py-3">
        <NavItem icon={<House />} label={t('nav.home')} active={is('home')} onClick={() => navigate({ view: 'home' })} />

        <div className="pt-2">
          {CATEGORIES.map(({ id, icon: Icon }) => {
            const items = plugins.filter((p) => p.category === id)
            // 分类默认展开含工具的分类；用户手动折叠状态持久化到 SQLite
            const open = collapsed[id] === undefined ? items.length > 0 : !collapsed[id]
            return (
              <div key={id} className="py-0.5">
                <button
                  type="button"
                  onClick={() => toggleCategory(id)}
                  aria-expanded={open}
                  className="flex h-9 w-full items-center gap-3 rounded-control px-3 text-[13px] text-fg outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <Icon className="size-4 text-fg-muted" />
                  <span className="flex-1 text-left">{t(`category.${id}`)}</span>
                  <ChevronDown
                    className={cn('size-3.5 text-fg-subtle transition-transform duration-150', !open && '-rotate-90')}
                  />
                </button>
                {open && (
                  <div className="mt-0.5 space-y-0.5">
                    {items.length === 0 && (
                      <p className="py-1.5 pl-12 text-xs text-fg-subtle">{t('sidebar.emptyCategory')}</p>
                    )}
                    {items.map((manifest) => (
                      <NavItem
                        key={manifest.id}
                        indent
                        icon={<ToolGlyph manifest={manifest} className="size-4" />}
                        label={text(manifest, manifest.name)}
                        active={activeTool === manifest.id}
                        onClick={() => openTool(manifest.id)}
                      />
                    ))}
                  </div>
                )}
              </div>
            )
          })}
        </div>

        <div className="mx-3 my-3 h-px bg-border" />
        <NavItem
          icon={<Star />}
          label={t('nav.favorites')}
          active={is('favorites')}
          onClick={() => navigate({ view: 'favorites' })}
        />
        <NavItem
          icon={<Clock3 />}
          label={t('nav.recent')}
          active={is('recent')}
          onClick={() => navigate({ view: 'recent' })}
        />
      </nav>

      <footer className="flex h-14 shrink-0 items-center border-t border-border px-3">
        <button
          type="button"
          onClick={() => navigate({ view: 'about' })}
          aria-current={is('about') ? 'page' : undefined}
          className={cn(
            'flex h-10 w-full items-center gap-2.5 rounded-control px-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
            is('about') ? 'bg-hover' : 'hover:bg-hover',
          )}
        >
          <Logo className="size-6" />
          <span className="text-[13px] text-fg">ToolForge</span>
          <span className="ml-auto text-xs text-fg-subtle">v{info?.version}</span>
        </button>
      </footer>
    </aside>
  )
}
