import { createContext, useCallback, useContext, useMemo, type ReactNode } from 'react'
import { callPlugin } from './call'
import type { Manifest } from './types'

interface PluginContextValue {
  manifest: Manifest
}

const PluginContext = createContext<PluginContextValue | null>(null)

export function PluginProvider({ manifest, children }: { manifest: Manifest; children: ReactNode }) {
  const value = useMemo(() => ({ manifest }), [manifest])
  return <PluginContext.Provider value={value}>{children}</PluginContext.Provider>
}

/** 插件 UI 内使用：拿到自身 manifest，并以自身身份调用后端 */
export function usePlugin() {
  const context = useContext(PluginContext)
  if (!context) throw new Error('usePlugin must be used inside <PluginProvider>')
  const { manifest } = context
  const call = useCallback(
    <T,>(fn: string, args?: object) => callPlugin<T>(manifest.id, fn, args),
    [manifest.id],
  )
  return { manifest, call }
}
