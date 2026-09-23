import type { Locale } from '../i18n'

export type ThemeMode = 'light' | 'dark' | 'system'

export interface Prefs {
  theme: ThemeMode
  locale: Locale | null
  /** 侧边栏各分类的折叠状态 */
  collapsed: Record<string, boolean>
}

interface BootData {
  prefs?: Partial<Prefs> | null
  os?: string
}

declare global {
  interface Window {
    __TF_BOOT__?: BootData
  }
}

/** Rust 在页面脚本执行前注入的启动数据（偏好设置来自 SQLite） */
export const boot: { prefs: Partial<Prefs>; os: string } = {
  prefs: window.__TF_BOOT__?.prefs ?? {},
  os: window.__TF_BOOT__?.os ?? 'unknown',
}

export const isMac = boot.os === 'macos'
