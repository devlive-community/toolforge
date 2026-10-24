import type { Locale } from '../i18n'

export type ThemeMode = 'light' | 'dark' | 'system'

/** 默认的全局快捷键，与 Rust 侧 launcher::DEFAULT_SHORTCUT 一致 */
export const DEFAULT_SHORTCUT = 'Alt+Space'

export interface Prefs {
  theme: ThemeMode
  locale: Locale | null
  /** 侧边栏各分类的折叠状态 */
  collapsed: Record<string, boolean>
  /** 启动时自动检查更新 */
  autoUpdate: boolean
  /** 用户选择跳过的版本 */
  skippedVersion: string | null
  /** 退出前弹出确认（Rust 侧也会读取该值） */
  confirmQuit: boolean
  /** 打开命令面板时按剪贴板内容推荐工具 */
  clipboardSuggest: boolean
  /** 唤起窗口与命令面板的全局快捷键；null 表示关闭（Rust 启动时读取） */
  globalShortcut: string | null
  /** 关闭窗口后留在菜单栏 / 系统托盘中运行（Rust 读取） */
  runInBackground: boolean
  /** 在侧边栏、首页与命令面板中隐藏的工具 */
  hiddenTools: string[]
  /** 用户已看过的工具版本，用于在工具市场标出新增与更新；首次启动时为 null */
  seenVersions: Record<string, string> | null
  /** 在线插件同时下载安装的最大数量（1–5，默认 3） */
  maxConcurrentInstalls: number
}

export const DEFAULT_CONCURRENT_INSTALLS = 3
export const MAX_CONCURRENT_INSTALLS = 5

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
