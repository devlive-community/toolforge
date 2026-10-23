'use client'

import { useRef, useState } from 'react'
import { ArrowDownToLine, X } from 'lucide-react'
import Link from 'next/link'
import { getDownloadGroups } from '@/lib/downloads'
import { release, RELEASES_URL, type Lang } from '@/lib/catalog'
import { dict } from '@/lib/dict'
import { siteCopy } from '@/lib/site-copy'

export function DownloadDialog({ lang, compact = false, label }: { lang: Lang; compact?: boolean; label?: string }) {
  const dialog = useRef<HTMLDialogElement>(null)
  const groups = getDownloadGroups()
  const [platform, setPlatform] = useState(groups[0]?.name ?? 'macos')
  const t = siteCopy[lang]
  const group = groups.find((item) => item.name === platform)
  return (
    <>
      <button type="button" className={`site-primary${compact ? ' site-small' : ''}`} onClick={() => dialog.current?.showModal()}>
        {!compact && <ArrowDownToLine aria-hidden="true" />}
        {label ?? (compact ? t.download : t.downloadDesktop)}
        {compact && <ArrowDownToLine aria-hidden="true" />}
      </button>
      <dialog
        ref={dialog}
        className="site-download-dialog"
        aria-label={t.downloadDesktop}
        onClick={(e) => {
          if (e.target === e.currentTarget) dialog.current?.close()
        }}
      >
        <section className="site-dialog">
          <div className="site-dialog-head">
            <h2>{lang === 'zh' ? '给桌面，添一套好工具。' : 'A few good tools for your desktop.'}</h2>
            <button type="button" className="site-icon-btn" onClick={() => dialog.current?.close()} aria-label={t.close}>
              <X aria-hidden="true" />
            </button>
          </div>
          <p>{lang === 'zh' ? '选择系统和架构，下载对应的正式安装包。' : 'Choose your operating system and architecture.'}</p>
          {groups.length > 0 ? (
            <>
              <div className="site-os-tabs" role="group" aria-label={lang === 'zh' ? '操作系统' : 'Operating system'}>
                {groups.map((item) => (
                  <button
                    key={item.name}
                    type="button"
                    aria-pressed={platform === item.name}
                    className={platform === item.name ? 'site-os-active' : ''}
                    onClick={() => setPlatform(item.name)}
                  >
                    {dict.download[item.name][lang]}
                  </button>
                ))}
              </div>
              {group?.items.map((item) => (
                <a key={item.url} className="site-download-option" href={item.url}>
                  <span>
                    {dict.download[item.labelKey][lang]}
                    <small>{dict.download[group.name][lang]}</small>
                  </span>
                  <ArrowDownToLine aria-hidden="true" />
                </a>
              ))}
            </>
          ) : (
            <a className="site-primary" href={`${RELEASES_URL}/latest`} target="_blank" rel="noreferrer">
              {dict.download.other[lang]}
              <ArrowDownToLine aria-hidden="true" />
            </a>
          )}
          <p className="site-download-version">
            {!release.fallback && release.version
              ? `${dict.download.version[lang]} v${release.version}`
              : lang === 'zh'
                ? '当前无法确认安装包，请查看正式发布页。'
                : 'Installer details are unavailable. Check the official releases.'}
          </p>
          <Link className="site-link" href={`/${lang}/guide/start/`} onClick={() => dialog.current?.close()}>
            {lang === 'zh' ? '需要安装帮助？' : 'Need installation help?'}
          </Link>
        </section>
      </dialog>
      <noscript>
        <a className="site-link" href={`${RELEASES_URL}/latest`}>
          {dict.download.other[lang]}
        </a>
      </noscript>
    </>
  )
}
