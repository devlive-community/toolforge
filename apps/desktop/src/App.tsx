import { useEffect } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { Spinner, Toaster } from '@toolforge/ui'
import { CommandPalette } from './layout/CommandPalette'
import { Sidebar } from './layout/Sidebar'
import { TitleBar } from './layout/TitleBar'
import { useApp } from './stores/app'
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
    default:
      return <Home />
  }
}

export function App() {
  const ready = useApp((s) => s.ready)
  const load = useApp((s) => s.load)

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
      <Toaster />
    </div>
  )
}
