import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { Empty, Input, Kbd, Modal, cn } from '@toolforge/ui'
import { CornerDownLeft, Moon, Search, Settings, Star, Sun } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { isMac } from '../lib/boot'
import { useApp } from '../stores/app'
import { useIsDark, usePrefs } from '../stores/prefs'

interface Entry {
  key: string
  icon: ReactNode
  title: string
  subtitle?: string
  /** 用于匹配的文本（工具名称、描述、关键词等元数据） */
  terms: string
  run: () => void
}

export function CommandPalette() {
  const { t } = useTranslation()
  const text = usePluginText()
  const open = useApp((s) => s.paletteOpen)
  const setOpen = useApp((s) => s.setPaletteOpen)
  const plugins = useApp((s) => s.plugins)
  const openTool = useApp((s) => s.openTool)
  const navigate = useApp((s) => s.navigate)
  const setTheme = usePrefs((s) => s.setTheme)
  const isDark = useIsDark()
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setOpen(!useApp.getState().paletteOpen)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [setOpen])

  useEffect(() => {
    if (open) {
      setQuery('')
      setActive(0)
    }
  }, [open])

  const entries = useMemo<Entry[]>(() => {
    const tools = plugins.map((manifest) => {
      const title = text(manifest, manifest.name)
      const subtitle = text(manifest, manifest.description)
      return {
        key: manifest.id,
        icon: <ToolTile manifest={manifest} size="sm" />,
        title,
        subtitle,
        terms: [title, subtitle, t(`category.${manifest.category}`), ...manifest.keywords].join(' '),
        run: () => openTool(manifest.id),
      }
    })
    const actions: Entry[] = [
      {
        key: 'action.theme',
        icon: isDark ? <Sun /> : <Moon />,
        title: t(isDark ? 'palette.lightTheme' : 'palette.darkTheme'),
        terms: 'theme dark light 主题 暗色 亮色',
        run: () => {
          setTheme(isDark ? 'light' : 'dark')
          setOpen(false)
        },
      },
      {
        key: 'action.favorites',
        icon: <Star />,
        title: t('nav.favorites'),
        terms: 'favorites 收藏',
        run: () => navigate({ view: 'favorites' }),
      },
      {
        key: 'action.settings',
        icon: <Settings />,
        title: t('nav.settings'),
        terms: 'settings preferences 设置',
        run: () => navigate({ view: 'settings' }),
      },
    ]
    return [...tools, ...actions]
  }, [plugins, text, t, isDark, openTool, navigate, setTheme, setOpen])

  const needle = query.trim().toLowerCase()
  const results = needle ? entries.filter((e) => e.terms.toLowerCase().includes(needle)) : entries

  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'ArrowDown') setActive((i) => Math.min(i + 1, results.length - 1))
    else if (event.key === 'ArrowUp') setActive((i) => Math.max(i - 1, 0))
    else if (event.key === 'Enter') results[active]?.run()
    else return
    event.preventDefault()
  }

  return (
    <Modal open={open} onOpenChange={setOpen} position="top" size="lg" hideClose>
      <div className="border-b border-border p-3" onKeyDown={onKeyDown}>
        <Input
          autoFocus
          size="lg"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value)
            setActive(0)
          }}
          placeholder={t('palette.placeholder')}
          leading={<Search />}
          wrapperClassName="border-transparent bg-transparent hover:border-transparent focus-within:border-transparent focus-within:ring-0"
          aria-label={t('palette.placeholder')}
        />
      </div>
      <div ref={listRef} className="max-h-[50vh] overflow-y-auto p-2" role="listbox">
        {results.length === 0 && <Empty icon={<Search />} title={t('palette.noResults')} />}
        {results.map((entry, index) => (
          <div
            key={entry.key}
            data-index={index}
            role="option"
            aria-selected={index === active}
            onMouseMove={() => setActive(index)}
            onClick={entry.run}
            className={cn(
              'flex cursor-pointer items-center gap-3 rounded-control px-3 py-2',
              index === active ? 'bg-hover' : '',
            )}
          >
            <span className="flex size-8 items-center justify-center text-fg-muted [&_svg]:size-4">{entry.icon}</span>
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] font-medium text-fg">{entry.title}</p>
              {entry.subtitle && <p className="truncate text-xs text-fg-muted">{entry.subtitle}</p>}
            </div>
            {index === active && <CornerDownLeft className="size-4 text-fg-subtle" />}
          </div>
        ))}
      </div>
      <div className="flex items-center gap-4 border-t border-border px-4 py-2 text-xs text-fg-subtle">
        <span className="flex items-center gap-1">
          <Kbd>↑</Kbd>
          <Kbd>↓</Kbd>
          {t('palette.navigate')}
        </span>
        <span className="flex items-center gap-1">
          <Kbd>↵</Kbd>
          {t('palette.open')}
        </span>
        <span className="ml-auto flex items-center gap-1">
          <Kbd>{isMac ? '⌘' : 'Ctrl'}</Kbd>
          <Kbd>K</Kbd>
        </span>
      </div>
    </Modal>
  )
}
