import { useState } from 'react'
import { Button, Modal, Panel, Switch } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { EyeOff, Lock, Settings2, ShieldCheck, Trash2, X } from 'lucide-react'
import type { Status } from './types'

export function Settings({
  status,
  onEnable,
  onPermission,
  onInclude,
  onClear,
}: {
  status: Status
  onEnable: (enabled: boolean) => void
  onPermission: () => void
  onInclude: (id: string) => void
  onClear: () => Promise<void>
}) {
  const { t } = usePlugin()
  const [confirm, setConfirm] = useState(false)
  const [clearing, setClearing] = useState(false)

  return (
    <div className="flex max-w-3xl flex-col gap-4">
      <Panel title={t('settings.title')} icon={<Settings2 />}>
        <div className="flex items-center gap-4 border-b border-border px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13px] font-medium text-fg">{t('settings.enable')}</div>
            <div className="text-xs text-fg-muted">{t('settings.enableHint')}</div>
          </div>
          <Switch checked={status.enabled} onCheckedChange={onEnable} disabled={!!status.unsupported} />
        </div>
        {status.permission !== 'not_required' && (
          <div className="flex items-center gap-4 px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="text-[13px] font-medium text-fg">{t('settings.permission')}</div>
              <div className="text-xs text-fg-muted">
                {status.permission === 'granted' ? t('settings.permissionGranted') : t('settings.permissionHint')}
              </div>
            </div>
            {status.permission === 'granted' ? (
              <span className="flex items-center gap-1 text-xs text-success">
                <ShieldCheck className="size-3.5" />
                {t('settings.granted')}
              </span>
            ) : (
              <Button size="sm" onClick={onPermission}>
                {t('settings.grant')}
              </Button>
            )}
          </div>
        )}
      </Panel>

      <Panel title={t('settings.excluded')} icon={<EyeOff />}>
        {status.excluded.length === 0 ? (
          <p className="px-4 py-4 text-xs text-fg-muted">{t('settings.excludedEmpty')}</p>
        ) : (
          <ul className="flex flex-wrap gap-2 p-3">
            {status.excluded.map((app) => (
              <li key={app.id} className="flex items-center gap-1 rounded-control border border-border bg-surface-2 py-0.5 pr-0.5 pl-2 text-xs text-fg">
                {app.name}
                <Button variant="ghost" size="icon-sm" aria-label={t('settings.include', { name: app.name })} onClick={() => onInclude(app.id)}>
                  <X />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title={t('settings.privacy')} icon={<Lock />}>
        <ul className="flex list-disc flex-col gap-1.5 py-3 pr-4 pl-8 text-xs text-fg-muted">
          {(t('privacy', { returnObjects: true }) as string[]).map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
        <div className="flex items-center gap-4 border-t border-border px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13px] font-medium text-fg">{t('settings.clear')}</div>
            <div className="text-xs text-fg-muted">{t('settings.clearHint')}</div>
          </div>
          <Button size="sm" variant="outline" onClick={() => setConfirm(true)}>
            <Trash2 />
            {t('settings.clear')}
          </Button>
        </div>
      </Panel>

      <Modal
        open={confirm}
        onOpenChange={setConfirm}
        size="sm"
        title={t('settings.clearTitle')}
        description={t('settings.clearConfirm')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setConfirm(false)}>
              {t('cancel')}
            </Button>
            <Button
              variant="danger"
              loading={clearing}
              onClick={() => {
                setClearing(true)
                onClear().finally(() => {
                  setClearing(false)
                  setConfirm(false)
                })
              }}
            >
              {t('settings.clearAction')}
            </Button>
          </>
        }
      />
    </div>
  )
}
