import { useEffect, useRef, useState } from 'react'
import { usePlugin } from './context'
import type { AppError } from './types'

const DEBOUNCE_MS = 250

/**
 * 输入变化后防抖调用插件后端；只保留最后一次请求的结果，丢弃过期响应。
 * fn 或 args 为 null 时清空结果。
 */
export function useDebouncedCall<T>(fn: string | null, args: object | null, deps: unknown[]) {
  const { call } = usePlugin()
  const [result, setResult] = useState<T | null>(null)
  const [error, setError] = useState<AppError | null>(null)
  const [pending, setPending] = useState(false)
  const seq = useRef(0)

  useEffect(() => {
    const id = ++seq.current
    if (!fn || !args) {
      setResult(null)
      setError(null)
      setPending(false)
      return
    }
    setPending(true)
    const timer = setTimeout(() => {
      call<T>(fn, args)
        .then((value) => {
          if (id !== seq.current) return
          setResult(value)
          setError(null)
        })
        .catch((reason: AppError) => {
          if (id !== seq.current) return
          setResult(null)
          setError(reason)
        })
        .finally(() => {
          if (id === seq.current) setPending(false)
        })
    }, DEBOUNCE_MS)
    return () => clearTimeout(timer)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)

  return { result, error, pending }
}
