import { getCurrentWindow } from '@tauri-apps/api/window'
import { Badge, Button, Kbd, Tooltip, cn } from '@toolforge/ui'
import { ArrowUpCircle, History, Maximize2, Minus, Moon, Search, Settings, Star, Sun, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { isMac } from '../lib/boot'
import { useApp } from '../stores/app'
import { useIsDark, usePrefs } from '../stores/prefs'
import { useUpdate } from '../stores/update'
import { Logo } from './Logo'

export function TitleBar() {
  const { t } = useTranslation()
  const navigate = useApp((s) => s.navigate)
  const setPaletteOpen = useApp((s) => s.setPaletteOpen)
  const setTheme = usePrefs((s) => s.setTheme)
  const isDark = useIsDark()
  const version = useApp((s) => s.info?.version)
  const newVersion = useUpdate((s) => (s.status !== 'idle' && s.status !== 'latest' && s.status !== 'checking' ? s.info?.version : undefined))
  const openUpdate = useUpdate((s) => s.setDialogOpen)

  return (
    <header
      data-tauri-drag-region
      className="relative flex h-16 shrink-0 items-center gap-4 border-b border-border bg-surface pr-3"
    >
      <div data-tauri-drag-region className={cn('flex min-w-56 shrink-0 items-center gap-2.5', isMac ? 'pl-24' : 'pl-5')}>
        <Logo className="size-7" />
        <span data-tauri-drag-region className="text-[17px] font-semibold tracking-tight text-fg">
          ToolForge
        </span>
        {version && <Badge variant="primary">v{version}</Badge>}
      </div>

      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        className="flex h-10 w-full max-w-md items-center gap-2.5 rounded-full border border-border bg-surface-2 px-4 text-[13px] text-fg-subtle transition-colors hover:border-border-strong hover:bg-hover"
      >
        <Search className="size-4" />
        <span className="flex-1 text-left">{t('titlebar.search')}</span>
        <Kbd>{isMac ? '⌘' : 'Ctrl'}</Kbd>
        <Kbd>K</Kbd>
      </button>

      <div data-tauri-drag-region className="flex-1 self-stretch" />

      <nav className="flex items-center gap-1">
        {newVersion && (
          <Button variant="soft" size="sm" className="mr-1 rounded-full" onClick={() => openUpdate(true)}>
            <ArrowUpCircle />
            {t('update.badge', { version: newVersion })}
          </Button>
        )}
        <Tooltip content={t('titlebar.toggleTheme')}>
          <Button
            variant="ghost"
            size="icon-md"
            aria-label={t('titlebar.toggleTheme')}
            onClick={() => setTheme(isDark ? 'light' : 'dark')}
          >
            {isDark ? <Moon /> : <Sun />}
          </Button>
        </Tooltip>
        <span className="mx-1 h-5 w-px bg-border" />
        <Button variant="ghost" onClick={() => navigate({ view: 'history' })}>
          <History />
          {t('nav.history')}
        </Button>
        <Button variant="ghost" onClick={() => navigate({ view: 'favorites' })}>
          <Star />
          {t('nav.favorites')}
        </Button>
        <Button variant="ghost" onClick={() => navigate({ view: 'settings' })}>
          <Settings />
          {t('nav.settings')}
        </Button>
      </nav>

      {!isMac && (
        <div className="ml-2 flex items-center gap-0.5 border-l border-border pl-2">
          <Button variant="ghost" size="icon-md" aria-label={t('window.minimize')} onClick={() => getCurrentWindow().minimize()}>
            <Minus />
          </Button>
          <Button variant="ghost" size="icon-md" aria-label={t('window.maximize')} onClick={() => getCurrentWindow().toggleMaximize()}>
            <Maximize2 />
          </Button>
          <Button
            variant="ghost"
            size="icon-md"
            aria-label={t('window.close')}
            className="hover:bg-danger hover:text-on-tile"
            onClick={() => getCurrentWindow().close()}
          >
            <X />
          </Button>
        </div>
      )}
    </header>
  )
}
