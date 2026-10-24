import { Badge, Button, SegmentedControl, Select, Switch, Tooltip, toast } from '@toolforge/ui'
import { formatBytes, host, useCopy, useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { Check, ChevronRight, Copy, FolderOpen, PackageOpen, RefreshCw, SquareTerminal, Trash2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { SUPPORTED_LOCALES, resolveLocale, type Locale } from '../i18n'
import type { ThemeMode } from '../lib/boot'
import { useApp } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { useUpdate } from '../stores/update'
import { AgentSection } from './AgentSettings'
import { Page, Row, Section } from './Page'
import { ShortcutRecorder } from './ShortcutRecorder'

const LOCALE_LABELS: Record<Locale, string> = { 'zh-CN': '简体中文', 'en-US': 'English' }

/** 命令行工具的总入口，与各插件的 tfp-<插件> 一起列出 */
const CLI_ENTRY = 'tfp'

interface CliStatus {
  available: boolean
  installed: boolean
  dir: string | null
  commands: string[]
  pathHint: string | null
}

/** 命令行工具：在 ~/.toolforge/bin 生成 tfp 与 tfp-<插件>，安装或卸载插件后由后端自动刷新 */
function CliToolsSection() {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage()
  const { copy, copied } = useCopy()
  const [status, setStatus] = useState<CliStatus | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    invoke<CliStatus>('cli_status')
      .then(setStatus)
      .catch(() => {})
  }, [])

  const run = async (command: 'cli_install' | 'cli_uninstall') => {
    setBusy(true)
    try {
      const next = await invoke<CliStatus>(command)
      setStatus(next)
      toast.success(command === 'cli_install' ? t('settings.cli.installed', { count: next.commands.length + 1 }) : t('settings.cli.removed'))
    } catch (error) {
      toast.error(errorMessage(error))
    } finally {
      setBusy(false)
    }
  }

  if (!status) return null
  return (
    <Section title={t('settings.cli.title')}>
      <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
        <Row
          label={t('settings.cli.label')}
          hint={status.installed ? t('settings.cli.installedHint', { dir: status.dir }) : t('settings.cli.hint')}
          warning={status.available ? null : t('settings.cli.unavailable')}
        >
          {status.installed ? (
            <Button loading={busy} onClick={() => void run('cli_uninstall')}>
              <Trash2 />
              {t('settings.cli.uninstall')}
            </Button>
          ) : (
            <Button variant="primary" loading={busy} disabled={!status.available} onClick={() => void run('cli_install')}>
              <SquareTerminal />
              {t('settings.cli.install')}
            </Button>
          )}
        </Row>
        {status.installed && (
          <div className="space-y-3 px-5 py-4">
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="text-xs text-fg-muted">{t('settings.cli.commands')}</span>
              {[CLI_ENTRY, ...status.commands].map((command) => (
                <Badge key={command}>{command}</Badge>
              ))}
            </div>
            {status.pathHint && (
              <div className="space-y-1.5">
                <p className="text-xs text-fg-muted">{t('settings.cli.path')}</p>
                <div className="flex items-center gap-2 rounded-control border border-border bg-surface-2 px-3 py-2">
                  <code className="min-w-0 flex-1 truncate font-mono text-xs text-fg" data-selectable>
                    {status.pathHint}
                  </code>
                  <Tooltip content={t('settings.cli.copy')}>
                    <Button size="icon-sm" variant="ghost" aria-label={t('settings.cli.copy')} onClick={() => void copy(status.pathHint ?? '')}>
                      {copied ? <Check className="text-success" /> : <Copy />}
                    </Button>
                  </Tooltip>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </Section>
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
  const maxConcurrentInstalls = usePrefs((s) => s.maxConcurrentInstalls)
  const setMaxConcurrentInstalls = usePrefs((s) => s.setMaxConcurrentInstalls)
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
          <Row label={t('settings.maxConcurrentInstalls')} hint={t('settings.maxConcurrentInstallsHint')}>
            <Select<number>
              size="sm"
              className="w-24 shrink-0"
              value={maxConcurrentInstalls}
              aria-label={t('settings.maxConcurrentInstalls')}
              options={[1, 2, 3, 4, 5].map((value) => ({ value, label: t('settings.concurrentCount', { count: value }) }))}
              onValueChange={setMaxConcurrentInstalls}
            />
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
      <CliToolsSection />
      <AgentSection />
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
