'use client'

// 官网「一键安装」：对分发渠道为在线插件的工具，通过 toolforge:// 深链唤起本地
// ToolForge 桌面端，由应用自动完成下载、验签与安装。浏览器无法感知唤起是否成功，
// 点击后展示「没有反应？先下载桌面端」的兜底引导。

import { useState } from 'react'
import { CloudDownload } from 'lucide-react'
import { toolInstallUrl, type Lang } from '@/lib/catalog'
import styles from './InstallButton.module.css'

const copy = {
  zh: {
    install: '一键安装',
    installLong: '一键安装到 ToolForge',
    launching: '正在唤起 ToolForge…',
    fallbackBefore: '没有反应？先',
    fallbackLink: '下载 ToolForge 桌面端',
    fallbackAfter: '，再试一次。',
    fallbackTitle: '没有反应？需要先安装 ToolForge 桌面端',
  },
  en: {
    install: 'Install',
    installLong: 'One-click install to ToolForge',
    launching: 'Launching ToolForge…',
    fallbackBefore: 'Nothing happened? ',
    fallbackLink: 'Get the ToolForge app',
    fallbackAfter: ' first, then try again.',
    fallbackTitle: 'Nothing happened? Install the ToolForge desktop app first',
  },
} satisfies Record<Lang, Record<string, string>>

export function InstallButton({
  slug,
  lang,
  variant,
}: {
  slug: string
  lang: Lang
  variant: 'card' | 'primary'
}) {
  const t = copy[lang]
  const [launched, setLaunched] = useState(false)
  const install = () => {
    // 必须在用户手势内直接跳转，浏览器才会放行自定义协议
    window.location.href = toolInstallUrl(slug)
    setLaunched(true)
  }
  return (
    <span className={styles.wrap}>
      <button
        type="button"
        className={variant === 'card' ? styles.card : styles.primary}
        onClick={install}
        title={launched ? t.fallbackTitle : undefined}
      >
        <CloudDownload size={variant === 'card' ? 12 : 16} aria-hidden="true" />
        {launched ? t.launching : variant === 'card' ? t.install : t.installLong}
      </button>
      {launched && variant === 'primary' && (
        <span className={styles.fallback}>
          {t.fallbackBefore}
          <a href={`/${lang}/#download`}>{t.fallbackLink}</a>
          {t.fallbackAfter}
        </span>
      )}
    </span>
  )
}
