import { Button, Modal, Progress } from '@toolforge/ui'
import { useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { Download, RotateCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { usePrefs } from '../stores/prefs'
import { useUpdate } from '../stores/update'

const formatBytes = (bytes: number) =>
  new Intl.NumberFormat(undefined, { style: 'unit', unit: 'megabyte', maximumFractionDigits: 1 }).format(bytes / 1024 / 1024)

export function UpdateDialog() {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage()
  const { status, info, downloaded, total, error, dialogOpen, setDialogOpen, install, restart } = useUpdate()
  const skipVersion = usePrefs((s) => s.skipVersion)
  const busy = status === 'downloading' || status === 'installing'

  if (!info) return null
  const percent = total ? (downloaded / total) * 100 : null

  return (
    <Modal
      open={dialogOpen}
      onOpenChange={(open) => !busy && setDialogOpen(open)}
      title={t('update.available', { version: info.version })}
      description={t('update.current', { version: info.currentVersion })}
      hideClose={busy}
      closeLabel={t('common:close')}
      footer={
        status === 'ready' ? (
          <Button variant="primary" onClick={restart}>
            <RotateCw />
            {t('update.restart')}
          </Button>
        ) : (
          <>
            {!busy && (
              <Button
                variant="ghost"
                className="mr-auto"
                onClick={() => {
                  skipVersion(info.version)
                  setDialogOpen(false)
                }}
              >
                {t('update.skip')}
              </Button>
            )}
            {!busy && <Button onClick={() => setDialogOpen(false)}>{t('update.later')}</Button>}
            <Button variant="primary" loading={busy} onClick={install}>
              {!busy && <Download />}
              {t(status === 'error' ? 'update.retry' : 'update.install')}
            </Button>
          </>
        )
      }
    >
      <div className="space-y-4 px-5 pb-5">
        {info.notes && (
          <div className="max-h-64 overflow-auto rounded-control border border-border bg-surface-2 p-3">
            <p className="text-xs font-medium text-fg-muted">{t('update.notes')}</p>
            <p className="mt-1.5 text-[13px] leading-relaxed whitespace-pre-wrap text-fg" data-selectable>
              {info.notes}
            </p>
          </div>
        )}
        {(busy || status === 'ready') && (
          <div className="space-y-1.5">
            <Progress value={status === 'downloading' ? percent : 100} tone={status === 'ready' ? 'success' : 'primary'} />
            <p className="text-xs text-fg-muted">
              {status === 'downloading' &&
                (total
                  ? t('update.downloading', { done: formatBytes(downloaded), total: formatBytes(total) })
                  : t('update.downloadingUnknown', { done: formatBytes(downloaded) }))}
              {status === 'installing' && t('update.installing')}
              {status === 'ready' && t('update.ready')}
            </p>
          </div>
        )}
        {status === 'error' && error && <p className="text-xs text-danger">{errorMessage(error)}</p>}
      </div>
    </Modal>
  )
}
