'use client'

// 官网「一键安装」：对分发渠道为在线插件的工具，通过 toolforge:// 深链唤起本地
// ToolForge 桌面端，由应用自动完成下载、验签与安装。
// 浏览器不会告知唤起结果，只能观察页面是否失去焦点：一段时间内没有离开页面，
// 多半是没有装桌面端或版本太旧（0.2.0 起才注册 toolforge://），提示用户下载或升级。

import { useEffect, useRef, useState } from 'react'
import { CloudDownload } from 'lucide-react'
import { toolInstallUrl, type Lang } from '@/lib/catalog'
import styles from './InstallButton.module.css'

/** 等待浏览器切换到桌面端的时长 */
const LAUNCH_WAIT_MS = 1800
/** 之后仍在这段时间内留意页面失焦（用户在浏览器的确认框里点得慢），一旦离开就撤回提示 */
const LATE_LAUNCH_MS = 30_000

const copy = {
  zh: {
    install: '一键安装',
    installLong: '一键安装到 ToolForge',
    launching: '正在唤起 ToolForge…',
    failedShort: '需要 ToolForge 0.2.0+',
    failedBefore: '没有打开 ToolForge？一键安装需要桌面端 0.2.0 或更高版本。请先',
    failedLink: '下载或升级 ToolForge',
    failedAfter: '，或在应用内「工具市场」中搜索安装。',
  },
  en: {
    install: 'Install',
    installLong: 'One-click install to ToolForge',
    launching: 'Launching ToolForge…',
    failedShort: 'Needs ToolForge 0.2.0+',
    failedBefore: 'ToolForge did not open? One-click install needs the desktop app 0.2.0 or later. ',
    failedLink: 'Download or upgrade ToolForge',
    failedAfter: ', or install it from the in-app tool market.',
  },
} satisfies Record<Lang, Record<string, string>>

type Phase = 'idle' | 'launching' | 'failed'

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
  const [phase, setPhase] = useState<Phase>('idle')
  const cleanup = useRef<(() => void) | null>(null)

  useEffect(() => () => cleanup.current?.(), [])

  const install = () => {
    cleanup.current?.()
    let left = false
    const onLeave = () => {
      left = true
      setPhase('idle')
    }
    const onVisibility = () => document.visibilityState === 'hidden' && onLeave()
    window.addEventListener('blur', onLeave)
    document.addEventListener('visibilitychange', onVisibility)
    const decide = window.setTimeout(() => !left && setPhase('failed'), LAUNCH_WAIT_MS)
    const stop = window.setTimeout(() => cleanup.current?.(), LATE_LAUNCH_MS)
    cleanup.current = () => {
      window.removeEventListener('blur', onLeave)
      document.removeEventListener('visibilitychange', onVisibility)
      window.clearTimeout(decide)
      window.clearTimeout(stop)
      cleanup.current = null
    }
    // 必须在用户手势内直接跳转，浏览器才会放行自定义协议
    window.location.href = toolInstallUrl(slug)
    setPhase('launching')
  }

  const failedText = `${t.failedBefore}${t.failedLink}${t.failedAfter}`
  const label =
    phase === 'launching'
      ? t.launching
      : phase === 'failed' && variant === 'card'
        ? t.failedShort
        : variant === 'card'
          ? t.install
          : t.installLong

  return (
    <span className={styles.wrap}>
      <button
        type="button"
        className={variant === 'card' ? styles.card : styles.primary}
        onClick={install}
        title={phase === 'failed' ? failedText : undefined}
      >
        <CloudDownload size={variant === 'card' ? 12 : 16} aria-hidden="true" />
        {label}
      </button>
      {phase === 'failed' && variant === 'primary' && (
        <span className={styles.fallback} role="status">
          {t.failedBefore}
          <a href={`/${lang}/#download`}>{t.failedLink}</a>
          {t.failedAfter}
        </span>
      )}
    </span>
  )
}
