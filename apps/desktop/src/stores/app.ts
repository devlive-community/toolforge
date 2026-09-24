import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'
import type { Manifest } from '@toolforge/plugin-ui-sdk'

export type Route =
  | { view: 'home' }
  | { view: 'tool'; pluginId: string }
  | { view: 'favorites' }
  | { view: 'recent' }
  | { view: 'history' }
  | { view: 'settings' }
  | { view: 'about' }

export interface AppInfo {
  version: string
  os: string
  arch: string
  tauriVersion: string
  webviewVersion: string | null
  dataDir: string | null
}

interface AppState {
  ready: boolean
  info: AppInfo | null
  plugins: Manifest[]
  favorites: string[]
  recent: string[]
  route: Route
  paletteOpen: boolean
  /** 待交给工具的内容（带着内容打开工具时） */
  launch: { pluginId: string; text: string; label: string | null; seq: number } | null
  /** 退出确认框（Rust 请求的序号，0 表示未打开） */
  quitRequest: number
  setQuitRequest: (seq: number) => void
  load: () => Promise<void>
  navigate: (route: Route) => void
  openTool: (pluginId: string) => void
  openToolWith: (pluginId: string, text: string, label?: string) => void
  takeLaunch: (pluginId: string) => { text: string; label: string | null } | null
  toggleFavorite: (pluginId: string) => Promise<void>
  setPaletteOpen: (open: boolean) => void
}

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  info: null,
  plugins: [],
  favorites: [],
  recent: [],
  route: { view: 'home' },
  paletteOpen: false,
  launch: null,
  quitRequest: 0,
  setQuitRequest: (quitRequest) => set({ quitRequest }),

  load: async () => {
    const [info, plugins, favorites, recent] = await Promise.all([
      invoke<AppInfo>('app_info'),
      invoke<Manifest[]>('plugin_list'),
      invoke<string[]>('favorites_list'),
      invoke<string[]>('recent_list'),
    ])
    set({ info, plugins, favorites, recent, ready: true })
  },

  navigate: (route) => set({ route, paletteOpen: false }),

  openTool: (pluginId) => {
    set((state) => ({
      route: { view: 'tool', pluginId },
      paletteOpen: false,
      recent: [pluginId, ...state.recent.filter((id) => id !== pluginId)].slice(0, 12),
    }))
    invoke('recent_touch', { pluginId }).catch(() => {})
  },

  openToolWith: (pluginId, text, label) => {
    set((state) => ({ launch: { pluginId, text, label: label ?? null, seq: (state.launch?.seq ?? 0) + 1 } }))
    get().openTool(pluginId)
  },

  takeLaunch: (pluginId) => {
    const launch = get().launch
    if (launch?.pluginId !== pluginId) return null
    set({ launch: null })
    return { text: launch.text, label: launch.label }
  },

  toggleFavorite: async (pluginId) => {
    const favored = await invoke<boolean>('favorite_toggle', { pluginId })
    const rest = get().favorites.filter((id) => id !== pluginId)
    set({ favorites: favored ? [pluginId, ...rest] : rest })
  },

  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
}))

export const pluginById = (plugins: Manifest[], id: string) => plugins.find((p) => p.id === id)
