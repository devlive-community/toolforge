import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Tooltip, cn } from '@toolforge/ui'
import { Copy, Minus, Square, X } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

function ControlButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string
  onClick: () => void
  danger?: boolean
  children: ReactNode
}) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className={cn(
          'flex size-8 items-center justify-center rounded-control text-fg-muted outline-none transition-colors duration-150',
          'focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-4',
          danger ? 'hover:bg-danger hover:text-on-tile' : 'hover:bg-hover hover:text-fg',
        )}
      >
        {children}
      </button>
    </Tooltip>
  )
}

/** 应用自绘的窗口控制按钮（各平台统一，系统原生按钮已隐藏） */
export function WindowControls() {
  const { t } = useTranslation()
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    const win = getCurrentWindow()
    const sync = () => win.isMaximized().then(setMaximized).catch(() => {})
    sync()
    const off = win.onResized(sync)
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])

  return (
    <div className="ml-1 flex items-center gap-0.5 border-l border-border pl-2">
      <ControlButton label={t('window.minimize')} onClick={() => getCurrentWindow().minimize()}>
        <Minus />
      </ControlButton>
      <ControlButton
        label={t(maximized ? 'window.restore' : 'window.maximize')}
        onClick={() => getCurrentWindow().toggleMaximize()}
      >
        {maximized ? <Copy className="-scale-x-100" /> : <Square className="size-3.5!" />}
      </ControlButton>
      <ControlButton label={t('window.close')} onClick={() => getCurrentWindow().close()} danger>
        <X />
      </ControlButton>
    </div>
  )
}
