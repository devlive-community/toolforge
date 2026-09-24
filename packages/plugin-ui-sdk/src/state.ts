import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { toast } from '@toolforge/ui'
import { usePlugin } from './context'
import { toAppError } from './types'

const SAVE_DELAY_MS = 400

/** 读取插件私有状态（存放在 SQLite），不存在时返回 null */
export async function loadPluginState<T>(pluginId: string, key: string): Promise<T | null> {
  try {
    return await invoke<T | null>('plugin_state_get', { pluginId, key })
  } catch (error) {
    throw toAppError(error)
  }
}

/** 写入插件私有状态；传 null 删除 */
export async function savePluginState(pluginId: string, key: string, value: unknown): Promise<void> {
  try {
    await invoke('plugin_state_set', { pluginId, key, value })
  } catch (error) {
    throw toAppError(error)
  }
}

/**
 * 持久化的插件状态，例如编辑器草稿。修改后防抖写入 SQLite，组件卸载时立即写入未保存的修改。
 * 返回 [值, 设置函数, 是否已从存储中加载]；加载完成前的修改会覆盖已保存的值。
 */
export function usePluginState<T>(key: string, initial: T): [T, (value: T) => void, boolean] {
  const { manifest, errorMessage } = usePlugin()
  const [state, setState] = useState({ value: initial, loaded: false })
  const pending = useRef<{ value: T } | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  useEffect(() => {
    let alive = true
    loadPluginState<T>(manifest.id, key)
      .then((saved) => alive && setState((prev) => ({ value: pending.current || saved === null ? prev.value : saved, loaded: true })))
      .catch(() => alive && setState((prev) => ({ ...prev, loaded: true })))
    return () => {
      alive = false
    }
  }, [manifest.id, key])

  const flush = useCallback(() => {
    clearTimeout(timer.current)
    const next = pending.current
    pending.current = null
    if (next) savePluginState(manifest.id, key, next.value).catch((error) => toast.error(errorMessage(error)))
  }, [manifest.id, key, errorMessage])

  useEffect(() => flush, [flush])

  const set = useCallback(
    (value: T) => {
      pending.current = { value }
      setState((prev) => ({ ...prev, value }))
      clearTimeout(timer.current)
      timer.current = setTimeout(flush, SAVE_DELAY_MS)
    },
    [flush],
  )

  return [state.value, set, state.loaded]
}
