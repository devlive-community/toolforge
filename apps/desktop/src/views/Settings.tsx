import { SegmentedControl, Select } from '@toolforge/ui'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { SUPPORTED_LOCALES, resolveLocale, type Locale } from '../i18n'
import type { ThemeMode } from '../lib/boot'
import { useApp } from '../stores/app'
import { usePrefs } from '../stores/prefs'
import { Page, Section } from './Page'

const LOCALE_LABELS: Record<Locale, string> = { 'zh-CN': '简体中文', 'en-US': 'English' }

function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-5 py-4">
      <div>
        <p className="text-[13px] font-medium text-fg">{label}</p>
        {hint && <p className="mt-0.5 text-xs text-fg-muted">{hint}</p>}
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
      <Section title={t('settings.about')}>
        <div className="divide-y divide-border rounded-card border border-border bg-surface shadow-card">
          <Row label={t('settings.version')}>
            <span className="text-[13px] text-fg-muted" data-selectable>
              {info?.version} · {info?.os} / {info?.arch}
            </span>
          </Row>
          <Row label={t('settings.storage')} hint={t('settings.storageHint')}>
            <span className="text-[13px] text-fg-muted">SQLite</span>
          </Row>
        </div>
      </Section>
    </Page>
  )
}
