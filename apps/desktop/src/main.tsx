import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { invoke } from '@tauri-apps/api/core'
import { App } from './App'
import { initI18n } from './i18n'
import { registerPluginLocales } from './plugins/modules'
import { initialLocale } from './stores/prefs'
import './styles.css'

// 禁用 WebView 默认右键菜单（重新载入、检查元素等），复制粘贴等快捷键不受影响
window.addEventListener('contextmenu', (event) => event.preventDefault())

// 未捕获的前端错误转发到 Rust 终端输出
const report = (message: string) => invoke('app_log', { level: 'error', message }).catch(() => {})
window.addEventListener('error', (event) => report(`${event.message} @ ${event.filename}:${event.lineno}`))
window.addEventListener('unhandledrejection', (event) => report(`unhandled rejection: ${String(event.reason)}`))

await initI18n(initialLocale())
registerPluginLocales()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
