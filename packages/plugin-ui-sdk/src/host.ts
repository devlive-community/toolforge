import { invoke } from '@tauri-apps/api/core'
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
  clipboard: {
    readText: () => wrap(readText()),
    writeText: (text: string) => wrap(writeText(text)),
  },
  dialog: {
    /** 选择单个文件，取消时返回 null */
    openFile: (filters?: FileFilter[]) =>
      wrap(open({ multiple: false, directory: false, filters })) as Promise<string | null>,
    saveFile: (defaultPath?: string, filters?: FileFilter[]) => wrap(save({ defaultPath, filters })),
  },
  fs: {
    readText: (path: string) => wrap(invoke<string>('fs_read_text', { path })),
    writeText: (path: string, text: string) => wrap(invoke<void>('fs_write_text', { path, text })),
  },
}
