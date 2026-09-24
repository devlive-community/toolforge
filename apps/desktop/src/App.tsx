import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
import { Spinner, Toaster, toast } from '@toolforge/ui'
import { CommandPalette } from './layout/CommandPalette'
import { Sidebar } from './layout/Sidebar'
import { TaskCenter } from './layout/TaskCenter'
import { TitleBar } from './layout/TitleBar'
import { QuitDialog } from './layout/QuitDialog'
import { UpdateDialog } from './layout/UpdateDialog'
import { useApp } from './stores/app'
import { usePrefs } from './stores/prefs'
import { subscribeTasks, useTasks } from './stores/tasks'
import { useUpdate } from './stores/update'
import { About } from './views/About'
import { Collection } from './views/Collection'
import { History } from './views/History'
import { Home } from './views/Home'
import { Settings } from './views/Settings'
import { ToolView } from './views/ToolView'

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
  const ready = useApp((s) => s.ready)
  const load = useApp((s) => s.load)

  useEffect(() => {
    // 退出请求：先应答 Rust（否则超时后直接退出），再弹出确认框
    const offQuit = listen<number>('app://close-requested', ({ payload }) => {
      invoke('app_close_ack', { seq: payload }).catch(() => {})
      useApp.getState().setQuitRequest(payload)
    })
    // macOS 菜单中的「关于」「设置」
    const offNavigate = listen<'about' | 'settings'>('app://navigate', ({ payload }) => {
      useApp.getState().navigate({ view: payload })
    })
    return () => {
      offQuit.then((off) => off())
      offNavigate.then((off) => off())
    }
  }, [])

  useEffect(() => {
    invoke('app_menu_locale', { locale: i18n.language }).catch(() => {})
  }, [i18n.language])

  useEffect(() => {
    useTasks.getState().load().catch(() => {})
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
