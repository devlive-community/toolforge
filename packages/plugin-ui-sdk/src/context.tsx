import { createContext, useCallback, useContext, useEffect, useEffectEvent, useMemo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { callPlugin } from './call'
import { useErrorMessage } from './i18n'
import type { Manifest } from './types'

/** 宿主带着内容打开工具（例如命令面板的剪贴板推荐） */
export interface LaunchInput {
  /** 每次打开递增 */
  seq: number
  /** 取出内容；只能取一次 */
  take: () => string | null
}

interface PluginContextValue {
  manifest: Manifest
  launch?: LaunchInput
}

const PluginContext = createContext<PluginContextValue | null>(null)

export function PluginProvider({ manifest, launch, children }: { manifest: Manifest; launch?: LaunchInput; children: ReactNode }) {
  const value = useMemo(() => ({ manifest, launch }), [manifest, launch])
  return <PluginContext.Provider value={value}>{children}</PluginContext.Provider>
}

/**
 * 接收宿主传入的内容：工具被带着内容打开时调用 handler（每次打开只调用一次），
 * 插件在其中把内容填入自己的输入框。
 */
export function useLaunchInput(handler: (text: string) => void) {
  const launch = useContext(PluginContext)?.launch
  const seq = launch?.seq ?? 0
  const take = launch?.take
  const onInput = useEffectEvent(handler)
  useEffect(() => {
    const text = take?.()
    if (text != null) onInput(text)
  }, [seq, take])
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
