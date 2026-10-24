import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import type { Manifest } from '@toolforge/plugin-ui-sdk'
import { i18n, resolveLocale, type Locale } from '../i18n'
import { DEFAULT_CONCURRENT_INSTALLS, DEFAULT_SHORTCUT, MAX_CONCURRENT_INSTALLS, boot, type Prefs, type ThemeMode } from '../lib/boot'

interface PrefsState extends Prefs {
  systemDark: boolean
  setTheme: (theme: ThemeMode) => void
  setLocale: (locale: Locale) => void
  toggleCategory: (category: string) => void
  setAutoUpdate: (value: boolean) => void
  skipVersion: (version: string | null) => void
  setConfirmQuit: (value: boolean) => void
  setMaxConcurrentInstalls: (value: number) => void
  setClipboardSuggest: (value: boolean) => void
  /** 注册失败时抛出错误并保留原快捷键 */
  setGlobalShortcut: (value: string | null) => Promise<void>
  setRunInBackground: (value: boolean) => Promise<void>
  setToolHidden: (pluginId: string, hidden: boolean) => void
  /** 把这些工具的当前版本记为已看过 */
  markToolsSeen: (plugins: Manifest[]) => void
}

export type ToolStatus = 'new' | 'updated' | 'seen'

/** 相对用户上次看过的版本：新增、已更新或已看过（首次启动前全部视为已看过） */
export function toolStatus(manifest: Manifest, seen: Record<string, string> | null): ToolStatus {
  if (!seen) return 'seen'
  const version = seen[manifest.id]
  if (version === undefined) return 'new'
  return version === manifest.version ? 'seen' : 'updated'
}

const systemDark = matchMedia('(prefers-color-scheme: dark)')

/** 并发数限制在 1–5 之间，非法值回落到默认值 */
export function clampConcurrency(value: unknown): number {
  const n = typeof value === 'number' && Number.isFinite(value) ? Math.round(value) : DEFAULT_CONCURRENT_INSTALLS
  return Math.min(MAX_CONCURRENT_INSTALLS, Math.max(1, n))
}

export function applyTheme(theme: ThemeMode) {
  const dark = theme === 'dark' || (theme === 'system' && systemDark.matches)
  document.documentElement.classList.toggle('dark', dark)
}

let saveTimer: ReturnType<typeof setTimeout> | undefined

/** 偏好设置持久化到 SQLite（经 Rust），合并短时间内的多次修改 */
function persist(prefs: Prefs) {
  clearTimeout(saveTimer)
  saveTimer = setTimeout(() => {
    invoke('prefs_set', { prefs }).catch(() => {})
  }, 300)
}

const snapshot = (state: PrefsState): Prefs => ({
  theme: state.theme,
  locale: state.locale,
  collapsed: state.collapsed,
  autoUpdate: state.autoUpdate,
  skippedVersion: state.skippedVersion,
  confirmQuit: state.confirmQuit,
  clipboardSuggest: state.clipboardSuggest,
  globalShortcut: state.globalShortcut,
  runInBackground: state.runInBackground,
  hiddenTools: state.hiddenTools,
  seenVersions: state.seenVersions,
  maxConcurrentInstalls: state.maxConcurrentInstalls,
})

export const usePrefs = create<PrefsState>((set, get) => ({
  theme: boot.prefs.theme ?? 'system',
  locale: boot.prefs.locale ?? null,
  collapsed: boot.prefs.collapsed ?? {},
  autoUpdate: boot.prefs.autoUpdate ?? true,
  skippedVersion: boot.prefs.skippedVersion ?? null,
  confirmQuit: boot.prefs.confirmQuit ?? true,
  clipboardSuggest: boot.prefs.clipboardSuggest ?? true,
  globalShortcut: boot.prefs.globalShortcut === undefined ? DEFAULT_SHORTCUT : boot.prefs.globalShortcut,
  runInBackground: boot.prefs.runInBackground ?? false,
  hiddenTools: boot.prefs.hiddenTools ?? [],
  seenVersions: boot.prefs.seenVersions ?? null,
  maxConcurrentInstalls: clampConcurrency(boot.prefs.maxConcurrentInstalls),
  systemDark: systemDark.matches,
  setTheme: (theme) => {
    applyTheme(theme)
    set({ theme })
    persist(snapshot(get()))
  },
  setLocale: (locale) => {
    i18n.changeLanguage(locale)
    set({ locale })
    persist(snapshot(get()))
  },
  toggleCategory: (category) => {
    set((state) => ({ collapsed: { ...state.collapsed, [category]: !state.collapsed[category] } }))
    persist(snapshot(get()))
  },
  setAutoUpdate: (autoUpdate) => {
    set({ autoUpdate })
    persist(snapshot(get()))
  },
  skipVersion: (skippedVersion) => {
    set({ skippedVersion })
    persist(snapshot(get()))
  },
  setClipboardSuggest: (clipboardSuggest) => {
    set({ clipboardSuggest })
    persist(snapshot(get()))
  },
  setGlobalShortcut: async (globalShortcut) => {
    try {
      await invoke('launcher_set_shortcut', { shortcut: globalShortcut })
    } catch (error) {
      await invoke('launcher_set_shortcut', { shortcut: get().globalShortcut }).catch(() => {})
      throw error
    }
    set({ globalShortcut })
    persist(snapshot(get()))
  },
  setRunInBackground: async (runInBackground) => {
    await invoke('launcher_set_background', { enabled: runInBackground, locale: i18n.language })
    set({ runInBackground })
    // 关闭窗口时由 Rust 读取，需立即写入
    clearTimeout(saveTimer)
    await invoke('prefs_set', { prefs: snapshot(get()) }).catch(() => {})
  },
  setToolHidden: (pluginId, hidden) => {
    set((state) => ({ hiddenTools: hidden ? [...state.hiddenTools.filter((id) => id !== pluginId), pluginId] : state.hiddenTools.filter((id) => id !== pluginId) }))
    persist(snapshot(get()))
  },
  markToolsSeen: (plugins) => {
    const seenVersions = { ...(get().seenVersions ?? {}) }
    let changed = false
    for (const plugin of plugins) {
      if (seenVersions[plugin.id] !== plugin.version) {
        seenVersions[plugin.id] = plugin.version
        changed = true
      }
    }
    if (!changed && get().seenVersions) return
    set({ seenVersions })
    persist(snapshot(get()))
  },
  setMaxConcurrentInstalls: (value) => {
    set({ maxConcurrentInstalls: clampConcurrency(value) })
    persist(snapshot(get()))
  },
  setConfirmQuit: (confirmQuit) => {
    set({ confirmQuit })
    // 退出确认由 Rust 读取，需立即写入，避免刚修改就退出时仍按旧值处理
    clearTimeout(saveTimer)
    invoke('prefs_set', { prefs: snapshot(get()) }).catch(() => {})
  },
}))

systemDark.addEventListener('change', (event) => {
  usePrefs.setState({ systemDark: event.matches })
  if (usePrefs.getState().theme === 'system') applyTheme('system')
})

export const useIsDark = () =>
  usePrefs((s) => s.theme === 'dark' || (s.theme === 'system' && s.systemDark))

export const initialLocale = () => resolveLocale(usePrefs.getState().locale)

/** 未隐藏的工具（侧边栏、首页与命令面板使用） */
export const useHiddenTools = () => usePrefs((s) => s.hiddenTools)
