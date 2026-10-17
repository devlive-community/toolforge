import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
import { Spinner, Toaster, toast } from '@toolforge/ui'
import { useErrorMessage, type AppError } from '@toolforge/plugin-ui-sdk'
import { CommandPalette } from './layout/CommandPalette'
import { Sidebar } from './layout/Sidebar'
import { TaskCenter } from './layout/TaskCenter'
import { TitleBar } from './layout/TitleBar'
import { QuitDialog } from './layout/QuitDialog'
import { UpdateDialog } from './layout/UpdateDialog'
import { useApp, pluginById } from './stores/app'
import { usePrefs } from './stores/prefs'
import { subscribeTasks, useTasks } from './stores/tasks'
import { useUpdate } from './stores/update'
import { About } from './views/About'
import { Collection } from './views/Collection'
import { History } from './views/History'
import { Home } from './views/Home'
import { Market } from './views/Market'
import { Settings } from './views/Settings'
import { ToolView } from './views/ToolView'

/// 官网 toolforge:// 一键安装的结果事件（Rust 侧 deeplink.rs 发出）
interface ExternalInstallEvent {
  id: string
  outcome: 'installed' | 'updated' | 'uptodate' | 'builtin' | 'failed'
  version?: string
  names?: Record<string, string> | null
  error?: AppError | null
}

function Content() {
  const route = useApp((s) => s.route)
  switch (route.view) {
    case 'tool':
      return <ToolView key={route.pluginId} pluginId={route.pluginId} />
    case 'favorites':
      return <Collection kind="favorites" />
    case 'recent':
      return <Collection kind="recent" />
    case 'history':
      return <History />
    case 'market':
      return <Market />
    case 'settings':
      return <Settings />
    case 'about':
      return <About />
    default:
      return <Home />
  }
}

export function App() {
  const { t, i18n } = useTranslation()
  const errorMessage = useErrorMessage('common')
  const ready = useApp((s) => s.ready)
  const load = useApp((s) => s.load)

  useEffect(() => {
    // 退出请求：先应答 Rust（否则超时后直接退出），再弹出确认框
    const offQuit = listen<number>('app://close-requested', ({ payload }) => {
      invoke('app_close_ack', { seq: payload }).catch(() => {})
      useApp.getState().setQuitRequest(payload)
    })
    // 在线插件加载完成、安装或卸载后刷新插件列表
    const offPlugins = listen('plugins://changed', () => {
      void useApp.getState().reloadPlugins()
    })
    // 官网 toolforge:// 一键安装完成后的结果提示；名称优先用索引里的本地化名称，
    // 内置工具回退到插件列表中的名称
    const displayName = (names?: Record<string, string> | null, id?: string) => {
      const map = names && typeof names === 'object' ? names : {}
      const localized = map[i18n.language] ?? map['en-US'] ?? Object.values(map)[0]
      if (localized) return localized
      const manifest = pluginById(useApp.getState().plugins, id ?? '')
      if (!manifest) return id ?? ''
      return manifest.name.startsWith('i18n:') ? i18n.t(manifest.name.slice(5), { ns: manifest.id }) : manifest.name
    }
    const offInstall = listen<ExternalInstallEvent>('market://external-install', ({ payload }) => {
      const name = displayName(payload.names, payload.id)
      if (payload.error) toast.error(errorMessage(payload.error))
      else if (payload.outcome === 'uptodate') toast.info(t('market.online.uptodate', { name, id: payload.id }))
      else if (payload.outcome === 'builtin') toast.info(t('market.online.builtinTool', { name, id: payload.id }))
      else toast.success(t('market.online.installed', { name, id: payload.id, version: payload.version }))
    })
    // macOS 菜单中的「关于」「设置」
    const offNavigate = listen<'about' | 'settings'>('app://navigate', ({ payload }) => {
      useApp.getState().navigate({ view: payload })
    })
    // 全局快捷键：窗口已在前台时切换命令面板，否则重新打开（重新读取剪贴板）
    const offPalette = listen<boolean>('app://palette', ({ payload: focused }) => {
      const { paletteOpen, setPaletteOpen } = useApp.getState()
      if (!paletteOpen) setPaletteOpen(true)
      else {
        setPaletteOpen(false)
        if (!focused) requestAnimationFrame(() => setPaletteOpen(true))
      }
    })
    return () => {
      offQuit.then((off) => off())
      offPlugins.then((off) => off())
      offNavigate.then((off) => off())
      offPalette.then((off) => off())
      offInstall.then((off) => off())
    }
  }, [t, i18n, errorMessage])

  useEffect(() => {
    invoke('app_menu_locale', { locale: i18n.language }).catch(() => {})
  }, [i18n.language])

  useEffect(() => {
    useTasks.getState().loadPage(1).catch(() => {})
    const unlisten = subscribeTasks((event) => {
      if (event.status === 'succeeded') toast.success(t('tasks.done'))
      else if (event.status === 'failed') toast.error(t('tasks.failed'))
    })
    return () => {
      unlisten.then((off) => off())
    }
  }, [t])

  useEffect(() => {
    // 启动后稍等片刻再静默检查更新，避免与首屏加载争抢资源
    if (!usePrefs.getState().autoUpdate) return
    const timer = setTimeout(() => useUpdate.getState().check({ silent: true }), 3000)
    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    load().catch((error) => {
      invoke('app_log', { level: 'error', message: `load failed: ${JSON.stringify(error)}` }).catch(() => {})
    })
  }, [load])

  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-auto">
          {ready ? (
            <Content />
          ) : (
            <div className="flex h-full items-center justify-center text-fg-muted">
              <Spinner />
            </div>
          )}
        </main>
      </div>
      <CommandPalette />
      <UpdateDialog />
      <QuitDialog />
      <TaskCenter />
      <Toaster />
    </div>
  )
}
