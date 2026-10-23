'use client'

import { Moon, Sun } from 'lucide-react'

const KEY = 'tf-theme'

// 默认跟随系统外观；内联脚本在 hydration 前已设置好 class（见 layout.tsx），
// 图标显隐完全交给 CSS dark: 变体，无 state、无 effect，也不存在 hydration 闪烁
export function ThemeToggle({ label }: { label: string }) {
  const toggle = () => {
    const dark = document.documentElement.classList.toggle('dark')
    try {
      localStorage.setItem(KEY, dark ? 'dark' : 'light')
    } catch {
      // 存储不可用时仅本次会话生效
    }
  }

  return (
    <button type="button" onClick={toggle} aria-label={label} className="site-icon-btn">
      <Sun className="hidden size-4 dark:block" />
      <Moon className="size-4 dark:hidden" />
    </button>
  )
}
