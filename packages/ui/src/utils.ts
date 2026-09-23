import { clsx, type ClassValue } from 'clsx'
import { extendTailwindMerge } from 'tailwind-merge'

// 让 tailwind-merge 识别自定义 token，避免 text-fg 与 text-sm 等被错误合并
const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      color: [
        'bg', 'surface', 'surface-2', 'sidebar', 'elevated', 'overlay',
        'fg', 'fg-muted', 'fg-subtle', 'fg-on-primary',
        'border', 'border-strong', 'ring', 'hover', 'active',
        'primary', 'primary-hover', 'primary-soft', 'primary-fg',
        'success', 'success-soft', 'warning', 'warning-soft',
        'danger', 'danger-soft', 'info', 'info-soft',
      ],
      radius: ['control', 'popover', 'card', 'tile'],
      shadow: ['card', 'popover', 'modal', 'tile'],
      spacing: ['control-sm', 'control-md', 'control-lg'],
    },
  },
})

export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs))
