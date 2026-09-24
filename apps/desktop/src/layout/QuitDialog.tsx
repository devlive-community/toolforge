import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Button, Checkbox, Modal } from '@toolforge/ui'
import { LogOut, Minus, TriangleAlert } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useApp } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { useTasks } from '../stores/tasks'

/** 退出确认：关闭窗口、⌘Q、菜单退出都会先弹出此对话框 */
export function QuitDialog() {
  const { t } = useTranslation()
  const request = useApp((s) => s.quitRequest)
  const setRequest = useApp((s) => s.setQuitRequest)
  const setConfirmQuit = usePrefs((s) => s.setConfirmQuit)
  const running = useTasks((s) => s.records.filter((task) => task.status === 'running').length)
  const [dontAsk, setDontAsk] = useState(false)

  const close = () => {
    setRequest(0)
    setDontAsk(false)
  }

  const quit = () => {
    if (dontAsk) setConfirmQuit(false)
    invoke('app_quit').catch(() => {})
  }

  const minimize = () => {
    close()
    getCurrentWindow().minimize().catch(() => {})
  }

  return (
    <Modal
      open={request > 0}
      onOpenChange={(open) => !open && close()}
      size="sm"
      title={t('quit.title')}
      description={t('quit.description')}
      closeLabel={t('common:close')}
      footer={
        <>
          <Button variant="ghost" className="mr-auto" onClick={minimize}>
            <Minus />
            {t('quit.minimize')}
          </Button>
          <Button onClick={close}>{t('common:cancel')}</Button>
          <Button variant="danger" onClick={quit} autoFocus>
            <LogOut />
            {t('quit.confirm')}
          </Button>
        </>
      }
    >
      <div className="space-y-3 px-5 pb-5">
        {running > 0 && (
          <p className="flex items-start gap-2 rounded-control bg-warning-soft px-3 py-2 text-xs text-warning">
            <TriangleAlert className="mt-px size-3.5 shrink-0" />
            {t('quit.running', { count: running })}
          </p>
        )}
        <Checkbox checked={dontAsk} onCheckedChange={setDontAsk}>
          {t('quit.dontAsk')}
        </Checkbox>
      </div>
    </Modal>
  )
}
