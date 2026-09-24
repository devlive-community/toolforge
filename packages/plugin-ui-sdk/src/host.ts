import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager'
import { open, save } from '@tauri-apps/plugin-dialog'
import { toAppError } from './types'

export interface FileFilter {
  name: string
  extensions: string[]
}

const wrap = async <T,>(promise: Promise<T>): Promise<T> => {
  try {
    return await promise
  } catch (error) {
    throw toAppError(error)
  }
}

/**
 * 宿主提供给插件的能力。插件不直接使用 Tauri API，便于宿主统一做权限控制，
 * 也让插件在后续迁移到沙箱运行时无需改动。
 */
export const host = {
  /** 用系统浏览器打开 http(s) 链接 */
  openUrl: (url: string) => wrap(invoke<void>('app_open_url', { url })),
  /** 在系统文件管理器中显示文件 */
  revealPath: (path: string) => wrap(invoke<void>('app_reveal_path', { path })),
  clipboard: {
    readText: () => wrap(readText()),
    writeText: (text: string) => wrap(writeText(text)),
  },
  dialog: {
    /** 选择单个文件，取消时返回 null */
    openFile: (filters?: FileFilter[]) =>
      wrap(open({ multiple: false, directory: false, filters })) as Promise<string | null>,
    /** 选择多个文件，取消时返回空数组 */
    openFiles: async (filters?: FileFilter[]) => {
      const picked = await wrap(open({ multiple: true, directory: false, filters }))
      return picked ?? []
    },
    /** 选择目录，取消时返回 null */
    openDirectory: () => wrap(open({ multiple: false, directory: true })) as Promise<string | null>,
    saveFile: (defaultPath?: string, filters?: FileFilter[]) => wrap(save({ defaultPath, filters })),
  },
  /**
   * 监听从系统拖入窗口的文件；返回取消监听函数。
   * `over` 表示文件正悬停在窗口上方，用于展示拖放高亮。
   */
  onFileDrop: async (handlers: { drop: (paths: string[]) => void; over?: (hovering: boolean) => void }) =>
    getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === 'drop') {
        handlers.over?.(false)
        handlers.drop(payload.paths)
      } else if (payload.type === 'enter' || payload.type === 'over') {
        handlers.over?.(true)
      } else {
        handlers.over?.(false)
      }
    }),
  fs: {
    readText: (path: string) => wrap(invoke<string>('fs_read_text', { path })),
    writeText: (path: string, text: string) => wrap(invoke<void>('fs_write_text', { path, text })),
  },
}
