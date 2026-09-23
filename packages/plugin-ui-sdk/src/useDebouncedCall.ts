import { useEffect, useState } from 'react'
import { usePlugin } from './context'
import type { AppError } from './types'

const DEBOUNCE_MS = 250

interface Settled<T> {
  token: object
  result: T | null
  error: AppError | null
}

/**
 * 输入变化后防抖调用插件后端；只保留最后一次请求的结果，丢弃过期响应。
 * fn 或 args 为 null 时返回空结果。等待新结果期间保留上一次结果，避免界面闪烁。
 */
export function useDebouncedCall<T>(fn: string | null, args: object | null, deps: unknown[]) {
  const { call } = usePlugin()
  const active = fn !== null && args !== null
  // deps 浅比较变化时生成新的请求标识（渲染期派生状态）
  const [request, setRequest] = useState({ deps, token: {} as object })
  if (request.deps.length !== deps.length || request.deps.some((d, i) => !Object.is(d, deps[i]))) {
    setRequest({ deps, token: {} })
  }
  const token = request.token
  const [settled, setSettled] = useState<Settled<T> | null>(null)

  useEffect(() => {
    if (!active) return
    let stale = false
    const timer = setTimeout(() => {
      call<T>(fn, args)
        .then((result) => !stale && setSettled({ token, result, error: null }))
        .catch((error: AppError) => !stale && setSettled({ token, result: null, error }))
    }, DEBOUNCE_MS)
    return () => {
      stale = true
      clearTimeout(timer)
    }
    // 只在请求标识变化时重新请求
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, active])

  if (!active) return { result: null, error: null, pending: false }
  return {
    result: settled?.result ?? null,
    error: settled?.error ?? null,
    pending: settled?.token !== token,
  }
}
