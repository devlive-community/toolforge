import { createContext, useCallback, useContext, useMemo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { callPlugin } from './call'
import { useErrorMessage } from './i18n'
import type { Manifest } from './types'

interface PluginContextValue {
  manifest: Manifest
}

const PluginContext = createContext<PluginContextValue | null>(null)

export function PluginProvider({ manifest, children }: { manifest: Manifest; children: ReactNode }) {
  const value = useMemo(() => ({ manifest }), [manifest])
  return <PluginContext.Provider value={value}>{children}</PluginContext.Provider>
}

/**
 * 插件 UI 内使用：拿到自身 manifest、以自身身份调用后端，
 * 以及绑定到插件命名空间（回退 common）的翻译函数。
 */
export function usePlugin() {
  const context = useContext(PluginContext)
  if (!context) throw new Error('usePlugin must be used inside <PluginProvider>')
  const { manifest } = context
  const { t } = useTranslation([manifest.id, 'common'])
  const errorMessage = useErrorMessage(manifest.id)
  const call = useCallback(
    <T,>(fn: string, args?: object) => callPlugin<T>(manifest.id, fn, args),
    [manifest.id],
  )
  return { manifest, call, t, errorMessage }
}
