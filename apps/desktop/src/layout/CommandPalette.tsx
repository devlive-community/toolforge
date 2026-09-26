import { Fragment, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Empty, Input, Kbd, Modal, cn } from '@toolforge/ui'
import { ClipboardList, CornerDownLeft, Moon, Search, Settings, Star, Sun } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { ToolTile } from '../plugins/ToolIcon'
import { usePluginText } from '../plugins/usePluginText'
import { isMac } from '../lib/boot'
import { useApp } from '../stores/app'
import { useIsDark, usePrefs } from '../stores/prefs'

interface Entry {
  key: string
  /** 分组：剪贴板推荐排在最前 */
  group: 'clipboard' | 'tools'
  icon: ReactNode
  title: string
  subtitle?: string
  /** 用于匹配的文本（工具名称、描述、关键词等元数据） */
  terms: string
  run: () => void
}

interface ClipboardSuggestions {
  text: string | null
  preview: string
  /** 剪贴板是图片时的宽高 */
  image: [number, number] | null
  suggestions: { pluginId: string; score: number; label: string; params?: Record<string, unknown> }[]
}

/** 剪贴板推荐最多展示的条数 */
const MAX_SUGGESTIONS = 4

export function CommandPalette() {
  const open = useApp((s) => s.paletteOpen)
  const setOpen = useApp((s) => s.setPaletteOpen)

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

  // 内容只在打开时挂载，每次打开都是全新的搜索状态
  return (
    <Modal open={open} onOpenChange={setOpen} position="top" size="lg" hideClose>
      <PaletteContent />
    </Modal>
  )
}

function PaletteContent() {
  const { t } = useTranslation()
  const text = usePluginText()
  const setOpen = useApp((s) => s.setPaletteOpen)
  const plugins = useApp((s) => s.plugins)
  const openTool = useApp((s) => s.openTool)
  const openToolWith = useApp((s) => s.openToolWith)
  const clipboardSuggest = usePrefs((s) => s.clipboardSuggest)
  const [clip, setClip] = useState<ClipboardSuggestions | null>(null)
  const navigate = useApp((s) => s.navigate)
  const setTheme = usePrefs((s) => s.setTheme)
  const isDark = useIsDark()
  const [query, setQuery] = useState('')
  const [active, setActive] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // 打开面板时在 Rust 侧读取剪贴板并识别内容
  useEffect(() => {
    if (!clipboardSuggest) return
    let alive = true
    invoke<ClipboardSuggestions>('clipboard_suggestions')
      .then((result) => alive && setClip(result))
      .catch(() => {})
    return () => {
      alive = false
    }
  }, [clipboardSuggest])

  const suggestions = useMemo<Entry[]>(() => {
    if (!clip) return []
    // 图片由插件自行读取剪贴板，传空文本
    const content = clip.image ? '' : clip.text
    if (content == null || (!content && !clip.image)) return []
    return clip.suggestions.slice(0, MAX_SUGGESTIONS).flatMap((suggestion) => {
      const manifest = plugins.find((p) => p.id === suggestion.pluginId)
      if (!manifest) return []
      const detail = t(`detect.${suggestion.label}`, { ns: manifest.id, ...suggestion.params, defaultValue: '' })
      return [
        {
          key: `clipboard.${manifest.id}`,
          group: 'clipboard' as const,
          icon: <ToolTile manifest={manifest} size="sm" />,
          title: text(manifest, manifest.name),
          subtitle: detail || text(manifest, manifest.description),
          terms: '',
          run: () => openToolWith(manifest.id, content, suggestion.label),
        },
      ]
    })
  }, [clip, plugins, t, text, openToolWith])

  const entries = useMemo<Entry[]>(() => {
    const tools = plugins.map((manifest) => {
      const title = text(manifest, manifest.name)
      const subtitle = text(manifest, manifest.description)
      return {
        key: manifest.id,
        group: 'tools' as const,
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
        group: 'tools',
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
        group: 'tools',
        icon: <Star />,
        title: t('nav.favorites'),
        terms: 'favorites 收藏',
        run: () => navigate({ view: 'favorites' }),
      },
      {
        key: 'action.settings',
        group: 'tools',
        icon: <Settings />,
        title: t('nav.settings'),
        terms: 'settings preferences 设置',
        run: () => navigate({ view: 'settings' }),
      },
    ]
    return [...tools, ...actions]
  }, [plugins, text, t, isDark, openTool, navigate, setTheme, setOpen])

  const needle = query.trim().toLowerCase()
  const results = needle ? entries.filter((e) => e.terms.toLowerCase().includes(needle)) : [...suggestions, ...entries]

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
    <>
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
          <Fragment key={entry.key}>
            {suggestions.length > 0 && !needle && (index === 0 || results[index - 1].group !== entry.group) && (
              <div className="flex items-center gap-2 px-3 pt-2 pb-1 text-[11px] font-semibold text-fg-subtle">
                {entry.group === 'clipboard' ? (
                  <>
                    <ClipboardList className="size-3.5" />
                    {t('palette.clipboard')}
                    <span className="min-w-0 flex-1 truncate font-mono font-normal" data-selectable>
                      {clip?.image ? t('palette.clipboardImage', { width: clip.image[0], height: clip.image[1] }) : clip?.preview}
                    </span>
                  </>
                ) : (
                  t('palette.tools')
                )}
              </div>
            )}
            <div
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
          </Fragment>
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
    </>
  )
}
