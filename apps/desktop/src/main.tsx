import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { getCurrentWindow } from '@tauri-apps/api/window'
import './styles.css'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <div className="flex h-full items-center justify-center">ToolForge</div>
  </StrictMode>,
)

// 窗口由 Rust 以隐藏状态创建，首帧渲染后再显示，避免白屏闪烁
requestAnimationFrame(() => {
  getCurrentWindow().show().catch(() => {})
})
