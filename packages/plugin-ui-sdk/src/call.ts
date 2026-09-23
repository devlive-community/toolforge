import { invoke } from '@tauri-apps/api/core'
import { toAppError } from './types'

/**
 * 调用插件后端函数。所有数据处理都在 Rust 侧完成，前端只负责传参和展示结果。
 * 失败时抛出 AppError。
 */
export async function callPlugin<T>(pluginId: string, fn: string, args: object = {}): Promise<T> {
  try {
    return await invoke<T>('plugin_call', { pluginId, function: fn, args })
  } catch (error) {
    throw toAppError(error)
  }
}
