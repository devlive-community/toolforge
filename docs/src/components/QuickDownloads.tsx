import Link from 'next/link'
import { Apple, ArrowDownToLine, ArrowRight, Monitor, Terminal } from 'lucide-react'
import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import { getDownloadGroups } from '@/lib/downloads'
import { dict } from '@/lib/dict'
import styles from './QuickDownloads.module.css'

const copy = {
  zh: { title: '下载最新版本', choose: '选择系统与架构，直接下载', unavailable: '查看最新发布与安装包', history: '更新日志' },
  en: { title: 'Download the latest release', choose: 'Choose your system and architecture', unavailable: 'View releases and installers', history: 'What’s new' },
}
const icons = { macos: Apple, windows: Monitor, linux: Terminal }

export function QuickDownloads({ lang }: { lang: Lang }) {
  const groups = getDownloadGroups()
  const t = copy[lang]
  return (
    <div className={styles.downloads} aria-label={t.choose}>
      {groups.length ? (
        <>
          <div className={styles.heading}><span>{t.title}</span><a href={release.url || RELEASES_URL} target="_blank" rel="noreferrer">v{release.version}<ArrowRight size={11} /></a></div>
          <div className={styles.platforms}>
            {groups.map((group) => {
              const Icon = icons[group.name]
              return (
                <div className={styles.platform} key={group.name}>
                  <span className={styles.platformName}><Icon size={15} aria-hidden="true" />{dict.download[group.name][lang]}</span>
                  <div className={styles.assets}>
                    {group.items.map((item) => <a key={item.labelKey} href={item.url} aria-label={`${dict.download[group.name][lang]} ${dict.download[item.labelKey][lang]} v${release.version}`}><span>{dict.download[item.labelKey][lang]}</span><ArrowDownToLine size={12} aria-hidden="true" /></a>)}
                  </div>
                </div>
              )
            })}
          </div>
        </>
      ) : <a className={styles.fallback} href={RELEASES_URL+'/latest'} target="_blank" rel="noreferrer"><ArrowDownToLine size={16} />{t.unavailable}<ArrowRight size={14} /></a>}
      <Link href={`/${lang}/changelog/`} className={styles.history}>{t.history}<ArrowRight size={11} /></Link>
    </div>
  )
}
