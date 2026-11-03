import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { invoke } from '@tauri-apps/api/core'
import { App } from './App'
import { initI18n } from './i18n'
import { registerPluginLocales } from './plugins/modules'
import { exposeSharedModules } from './plugins/shared'
import { initialLocale, applyTheme, usePrefs } from './stores/prefs'
import './styles.css'

// 禁用 WebView 默认右键菜单（重新载入、检查元素等），复制粘贴等快捷键不受影响
window.addEventListener('contextmenu', (event) => event.preventDefault())

// 未捕获的前端错误转发到 Rust 终端输出
const report = (message: string) => invoke('app_log', { level: 'error', message }).catch(() => {})
window.addEventListener('error', (event) => report(`${event.message} @ ${event.filename}:${event.lineno}`))
window.addEventListener('unhandledrejection', (event) => report(`unhandled rejection: ${String(event.reason)}`))

// 首帧主题由 Rust 注入脚本设置；这里按前端状态再对齐一次，
// 消除注入阶段与页面阶段 matchMedia 结果不一致造成的脱节（否则第一次切换主题会“空按”）
applyTheme(usePrefs.getState().theme ?? 'system')

await initI18n(initialLocale())
registerPluginLocales()
exposeSharedModules()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
