import { useEffect, useState } from 'react'
import { callPlugin, usePlugin } from '@toolforge/plugin-ui-sdk'
import type { ListKind, Page } from './types'

export const PAGE_SIZE = 50

/** 按页向后端查询导入、导出或函数列表 */
export function usePaged<T>(token: number, kind: ListKind, query: string, page: number, sort?: string) {
  const { manifest, errorMessage } = usePlugin()
  const [data, setData] = useState<{ key: string; page: Page<T> } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const key = `${token}:${kind}:${query}:${page}:${sort ?? ''}`

  useEffect(() => {
    let alive = true
    const timer = setTimeout(() => {
      callPlugin<Page<T>>(manifest.id, 'list', { token, kind, query, sort, offset: page * PAGE_SIZE, limit: PAGE_SIZE }).then(
        (result) => {
          if (!alive) return
          setData({ key, page: result })
          setError(null)
        },
        (reason) => alive && setError(errorMessage(reason)),
      )
    }, 120)
    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [manifest.id, errorMessage, token, kind, query, page, sort, key])

  return { page: data?.page ?? null, stale: data?.key !== key, error }
}
