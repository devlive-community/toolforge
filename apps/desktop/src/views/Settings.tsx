import { Button, SegmentedControl, Select, Switch, toast } from '@toolforge/ui'
import { formatBytes, host, useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { ChevronRight, FolderOpen, PackageOpen, RefreshCw } from 'lucide-react'
import { useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { SUPPORTED_LOCALES, resolveLocale, type Locale } from '../i18n'
import type { ThemeMode } from '../lib/boot'
import { useApp } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { useUpdate } from '../stores/update'
import { Page, Section } from './Page'
import { ShortcutRecorder } from './ShortcutRecorder'

const LOCALE_LABELS: Record<Locale, string> = { 'zh-CN': '简体中文', 'en-US': 'English' }

function Row({ label, hint, warning, children }: { label: string; hint?: string; warning?: string | null; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div>
        <p className="text-[13px] font-medium text-fg">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-fg-muted">{hint}</p>}
        {warning && <p className="mt-1 text-xs text-warning">{warning}</p>}
      </div>
      {children}
    </div>
  )
}

export function Settings() {
  const { t } = useTranslation()
  const theme = usePrefs((s) => s.theme)
  const setTheme = usePrefs((s) => s.setTheme)
  const locale = usePrefs((s) => s.locale)
  const setLocale = usePrefs((s) => s.setLocale)
  const info = useApp((s) => s.info)
  const confirmQuit = usePrefs((s) => s.confirmQuit)
  const setConfirmQuit = usePrefs((s) => s.setConfirmQuit)
  const clipboardSuggest = usePrefs((s) => s.clipboardSuggest)
  const setClipboardSuggest = usePrefs((s) => s.setClipboardSuggest)
  const globalShortcut = usePrefs((s) => s.globalShortcut)
  const setGlobalShortcut = usePrefs((s) => s.setGlobalShortcut)
  const runInBackground = usePrefs((s) => s.runInBackground)
  const setRunInBackground = usePrefs((s) => s.setRunInBackground)
  const navigate = useApp((s) => s.navigate)
  const autoUpdate = usePrefs((s) => s.autoUpdate)
  const setAutoUpdate = usePrefs((s) => s.setAutoUpdate)
  const update = useUpdate()
  const errorMessage = useErrorMessage()
  const [shortcutError, setShortcutError] = useState<AppError | null>(null)
  const [exporting, setExporting] = useState(false)

  // 启动时快捷键注册失败（例如被其他应用占用）需要提示用户更换
  useEffect(() => {
    invoke<{ error: AppError | null }>('launcher_status')
      .then((status) => setShortcutError(status.error))
      .catch(() => {})
  }, [globalShortcut])

  const exportDiagnostics = async () => {
    try {
      const date = new Date().toISOString().slice(0, 10)
      const path = await host.dialog.saveFile(`toolforge-diagnostics-${date}.zip`, [{ name: 'ZIP', extensions: ['zip'] }])
      if (!path) return
      setExporting(true)
      const result = await invoke<{ path: string; bytes: number }>('diagnostics_export', { path })
      toast.success(t('settings.diagnosticsDone', { size: formatBytes(result.bytes) }))
      host.revealPath(result.path).catch(() => {})
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setExporting(false)
    }
  }

  const updateHint = () => {
    switch (update.status) {
      case 'checking':
        return t('update.checking')
      case 'latest':
        return t('update.latest')
      case 'error':
        return update.error ? errorMessage(update.error) : undefined
      case 'available':
      case 'downloading':
      case 'installing':
      case 'ready':
        return update.info ? t('update.available', { version: update.info.version }) : undefined
      default:
        return t('update.hint')
    }
  }

  return (
    <Page title={t('nav.settings')}>
      <Section title={t('settings.appearance')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row label={t('settings.theme')} hint={t('settings.themeHint')}>
            <SegmentedControl<ThemeMode>
              value={theme}
              onValueChange={setTheme}
              aria-label={t('settings.theme')}
              options={[
                { value: 'light', label: t('settings.light') },
                { value: 'dark', label: t('settings.dark') },
                { value: 'system', label: t('settings.system') },
              ]}
            />
          </Row>
          <Row label={t('settings.language')}>
            <Select<Locale>
              value={resolveLocale(locale)}
              onValueChange={setLocale}
              className="w-40"
              aria-label={t('settings.language')}
              options={SUPPORTED_LOCALES.map((value) => ({ value, label: LOCALE_LABELS[value] }))}
            />
          </Row>
        </div>
      </Section>
      <Section title={t('settings.general')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row label={t('settings.confirmQuit')} hint={t('settings.confirmQuitHint')}>
            <Switch checked={confirmQuit} onCheckedChange={setConfirmQuit} aria-label={t('settings.confirmQuit')} />
          </Row>
          <Row label={t('settings.clipboardSuggest')} hint={t('settings.clipboardSuggestHint')}>
            <Switch checked={clipboardSuggest} onCheckedChange={setClipboardSuggest} aria-label={t('settings.clipboardSuggest')} />
          </Row>
        </div>
      </Section>
      <Section title={t('settings.launcher')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row
            label={t('settings.globalShortcut')}
            hint={t('settings.globalShortcutHint')}
            warning={shortcutError ? t('settings.shortcutFailed', { detail: errorMessage(shortcutError) }) : null}
          >
            <ShortcutRecorder value={globalShortcut} onChange={setGlobalShortcut} />
          </Row>
          <Row label={t('settings.runInBackground')} hint={t('settings.runInBackgroundHint')}>
            <Switch
              checked={runInBackground}
              onCheckedChange={(value) => setRunInBackground(value).catch((error) => toast.error(errorMessage(error)))}
              aria-label={t('settings.runInBackground')}
            />
          </Row>
        </div>
      </Section>
      <Section title={t('update.title')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row label={t('update.check')} hint={updateHint()}>
            {update.info && update.status !== 'checking' ? (
              <Button variant="primary" onClick={() => update.setDialogOpen(true)}>
                {t('update.view')}
              </Button>
            ) : (
              <Button loading={update.status === 'checking'} onClick={() => update.check()}>
                <RefreshCw />
                {t('update.check')}
              </Button>
            )}
          </Row>
          <Row label={t('update.auto')} hint={t('update.autoHint')}>
            <Switch checked={autoUpdate} onCheckedChange={setAutoUpdate} aria-label={t('update.auto')} />
          </Row>
        </div>
      </Section>
      <Section title={t('settings.about')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row label={t('settings.version')}>
            <span className="text-[13px] text-fg-muted" data-selectable>
              {info?.version} · {info?.os} / {info?.arch}
            </span>
          </Row>
          <Row label={t('about.title')} hint={t('about.hint')}>
            <Button variant="outline" onClick={() => navigate({ view: 'about' })}>
              {t('about.open')}
              <ChevronRight />
            </Button>
          </Row>
          <Row label={t('settings.storage')} hint={t('settings.storageHint')}>
            <span className="text-[13px] text-fg-muted">SQLite</span>
          </Row>
          <Row label={t('settings.diagnostics')} hint={t('settings.diagnosticsHint')}>
            <div className="flex items-center gap-2">
              {info?.logDir && (
                <Button onClick={() => host.revealPath(info.logDir!).catch((error) => toast.error(errorMessage(error)))}>
                  <FolderOpen />
                  {t('settings.openLogs')}
                </Button>
              )}
              <Button loading={exporting} onClick={exportDiagnostics}>
                <PackageOpen />
                {t('settings.diagnosticsExport')}
              </Button>
            </div>
          </Row>
        </div>
      </Section>
    </Page>
  )
}
