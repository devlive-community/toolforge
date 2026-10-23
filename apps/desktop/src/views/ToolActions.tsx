import { Button, Tooltip } from '@toolforge/ui'
import { Eye, EyeOff, Settings2, SquareArrowOutUpRight, Trash2 } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

/** 带 Tooltip 的图标按钮；禁用的按钮不触发指针事件，外面包一层才能显示原因 */
function IconAction({ label, disabled, onClick, children }: { label: string; disabled?: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <Tooltip content={label}>
      <span className="inline-flex">
        <Button size="icon-sm" variant="ghost" aria-label={label} disabled={disabled} onClick={onClick}>
          {children}
        </Button>
      </span>
    </Tooltip>
  )
}

export interface ToolActionsProps {
  name: string
  /** 工具声明了命令行或 AI Agent 接口 */
  supported: boolean
  onConfigure: () => void
  onOpen: () => void
  /** 不能打开的原因（插件未加载、需要更新应用…）；为空时可以打开 */
  openBlocked?: string | null
  hidden: boolean
  onToggleHidden: () => void
  /** 在线安装的工具可以卸载，内置工具不能 */
  removable: boolean
  busy?: boolean
  onUninstall: () => void
}

/** 工具的操作：设置、打开、显示/隐藏、卸载，全部是图标按钮 + Tooltip；不可用的置灰并说明原因 */
export function ToolActions({ name, supported, onConfigure, onOpen, openBlocked, hidden, onToggleHidden, removable, busy, onUninstall }: ToolActionsProps) {
  const { t } = useTranslation()
  return (
    <div className="flex items-center gap-0.5">
      <IconAction label={supported ? t('market.actions.configure', { name }) : t('market.actions.configureUnsupported')} disabled={!supported} onClick={onConfigure}>
        <Settings2 />
      </IconAction>
      <IconAction label={openBlocked ?? t('market.actions.open', { name })} disabled={!!openBlocked} onClick={onOpen}>
        <SquareArrowOutUpRight />
      </IconAction>
      <IconAction label={hidden ? t('market.actions.show', { name }) : t('market.actions.hide', { name })} onClick={onToggleHidden}>
        {hidden ? <EyeOff /> : <Eye />}
      </IconAction>
      <IconAction label={removable ? t('market.actions.uninstall', { name }) : t('market.actions.uninstallBuiltin')} disabled={!removable || busy} onClick={onUninstall}>
        <Trash2 />
      </IconAction>
    </div>
  )
}
