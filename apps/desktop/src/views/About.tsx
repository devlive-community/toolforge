import type { ReactNode } from 'react'
import { Badge, Button, cn, toast } from '@toolforge/ui'
import { host, useCopy, useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { Bug, Check, Copy, ExternalLink, FolderGit2, FolderOpen, History, RefreshCw, Scale } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Logo } from '../layout/Logo'
import { useApp } from '../stores/app'
import { useUpdate } from '../stores/update'

const REPO = 'https://github.com/devlive-community/toolforge'

const LINKS = [
  { key: 'repository', href: REPO, icon: <FolderGit2 /> },
  { key: 'releases', href: `${REPO}/releases`, icon: <History /> },
  { key: 'issues', href: `${REPO}/issues/new`, icon: <Bug /> },
  { key: 'license', href: 'https://opensource.org/licenses/MIT', icon: <Scale /> },
] as const

const CREDITS = [
  { name: 'Tauri', href: 'https://tauri.app', license: 'MIT / Apache-2.0' },
  { name: 'React', href: 'https://react.dev', license: 'MIT' },
  { name: 'Rust', href: 'https://www.rust-lang.org', license: 'MIT / Apache-2.0' },
  { name: 'tract', href: 'https://github.com/sonos/tract', license: 'MIT / Apache-2.0' },
  { name: 'U²-Net / IS-Net', href: 'https://github.com/danielgatis/rembg', license: 'Apache-2.0' },
  { name: 'Lucide', href: 'https://lucide.dev', license: 'ISC' },
]

function Card({ title, children, className }: { title: string; children: ReactNode; className?: string }) {
  return (
    <section className={cn('rounded-card border border-border bg-surface shadow-card', className)}>
      <h2 className="border-b border-border px-5 py-3 text-xs font-semibold tracking-wide text-fg-muted uppercase">{title}</h2>
      {children}
    </section>
  )
}

function InfoRow({ label, value, action }: { label: string; value: ReactNode; action?: ReactNode }) {
  return (
    <div className="flex items-center gap-4 px-5 py-3">
      <span className="w-28 shrink-0 text-[13px] text-fg-muted">{label}</span>
      <span className="min-w-0 flex-1 truncate font-mono text-[12.5px] text-fg" data-selectable>
        {value}
      </span>
      {action}
    </div>
  )
}

/** 应用内的关于页面（替代系统默认的关于面板） */
export function About() {
  const { t } = useTranslation()
  const info = useApp((s) => s.info)
  const tools = useApp((s) => s.plugins.length)
  const update = useUpdate()
  const errorMessage = useErrorMessage()
  const { copy, copied } = useCopy()

  const open = (href: string) => host.openUrl(href).catch((error) => toast.error(errorMessage(error)))
  const reveal = (path: string) => host.revealPath(path).catch((error) => toast.error(errorMessage(error)))

  const diagnostics = info
    ? [
        `ToolForge ${info.version}`,
        `${info.os} / ${info.arch}`,
        `Tauri ${info.tauriVersion}`,
        `WebView ${info.webviewVersion ?? '-'}`,
      ].join('\n')
    : ''

  const updateButton =
    update.info && update.status !== 'checking' ? (
      <Button variant="primary" onClick={() => update.setDialogOpen(true)}>
        {t('update.available', { version: update.info.version })}
      </Button>
    ) : (
      <Button loading={update.status === 'checking'} onClick={() => update.check()}>
        <RefreshCw />
        {update.status === 'latest' ? t('update.latest') : t('update.check')}
      </Button>
    )

  return (
    <div className="mx-auto w-full max-w-4xl animate-fade-in px-8 py-10">
      <header className="flex flex-col items-center text-center">
        <Logo className="size-20 rounded-[20px] shadow-card" />
        <h1 className="mt-5 text-3xl font-semibold tracking-tight text-fg">ToolForge</h1>
        <div className="mt-2 flex items-center gap-2">
          <Badge variant="success">v{info?.version}</Badge>
          <Badge>{t('about.tools', { count: tools })}</Badge>
        </div>
        <p className="mt-4 max-w-lg text-[13px] leading-relaxed text-fg-muted">{t('about.tagline')}</p>
        <div className="mt-5 flex flex-wrap justify-center gap-2">
          {updateButton}
          <Button variant="outline" onClick={() => open(REPO)}>
            <FolderGit2 />
            {t('about.links.repository')}
          </Button>
        </div>
      </header>

      <div className="mt-10 grid gap-4 md:grid-cols-2">
        <Card title={t('about.info')} className="md:col-span-2">
          <div className="divide-y divide-border">
            <InfoRow label={t('about.version')} value={info?.version} />
            <InfoRow label={t('about.platform')} value={info && `${info.os} / ${info.arch}`} />
            <InfoRow label={t('about.runtime')} value={info && `Tauri ${info.tauriVersion} · WebView ${info.webviewVersion ?? '-'}`} />
            <InfoRow
              label={t('about.dataDir')}
              value={info?.dataDir ?? '-'}
              action={
                info?.dataDir && (
                  <Button size="sm" variant="ghost" onClick={() => reveal(info.dataDir!)}>
                    <FolderOpen />
                    {t('about.reveal')}
                  </Button>
                )
              }
            />
          </div>
          <div className="flex justify-end border-t border-border px-5 py-3">
            <Button size="sm" variant="outline" disabled={!diagnostics} onClick={() => copy(diagnostics)}>
              {copied ? <Check className="text-success" /> : <Copy />}
              {t('about.copyInfo')}
            </Button>
          </div>
        </Card>

        <Card title={t('about.linksTitle')}>
          <div className="p-2">
            {LINKS.map((link) => (
              <button
                key={link.key}
                type="button"
                onClick={() => open(link.href)}
                className="group flex w-full items-center gap-3 rounded-control px-3 py-2.5 text-left outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-4"
              >
                <span className="text-fg-muted">{link.icon}</span>
                <span className="flex-1">
                  <span className="block text-[13px] font-medium text-fg">{t(`about.links.${link.key}`)}</span>
                  <span className="block text-xs text-fg-subtle">{t(`about.links.${link.key}Hint`)}</span>
                </span>
                <ExternalLink className="size-3.5! text-fg-subtle opacity-0 transition-opacity group-hover:opacity-100" />
              </button>
            ))}
          </div>
        </Card>

        <Card title={t('about.credits')}>
          <ul className="p-2">
            {CREDITS.map((credit) => (
              <li key={credit.name}>
                <button
                  type="button"
                  onClick={() => open(credit.href)}
                  className="flex w-full items-center justify-between gap-3 rounded-control px-3 py-2 text-left outline-none transition-colors hover:bg-hover focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <span className="text-[13px] text-fg">{credit.name}</span>
                  <span className="text-xs text-fg-subtle">{credit.license}</span>
                </button>
              </li>
            ))}
          </ul>
        </Card>
      </div>

      <footer className="mt-10 text-center text-xs text-fg-subtle">{t('about.copyright', { year: new Date().getFullYear() })}</footer>
    </div>
  )
}
