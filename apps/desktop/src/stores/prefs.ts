import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import { i18n, resolveLocale, type Locale } from '../i18n'
import { DEFAULT_SHORTCUT, boot, type Prefs, type ThemeMode } from '../lib/boot'

interface PrefsState extends Prefs {
  systemDark: boolean
  setTheme: (theme: ThemeMode) => void
  setLocale: (locale: Locale) => void
  toggleCategory: (category: string) => void
  setAutoUpdate: (value: boolean) => void
  skipVersion: (version: string | null) => void
  setConfirmQuit: (value: boolean) => void
  setClipboardSuggest: (value: boolean) => void
  /** 注册失败时抛出错误并保留原快捷键 */
  setGlobalShortcut: (value: string | null) => Promise<void>
  setRunInBackground: (value: boolean) => Promise<void>
}

const systemDark = matchMedia('(prefers-color-scheme: dark)')

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
