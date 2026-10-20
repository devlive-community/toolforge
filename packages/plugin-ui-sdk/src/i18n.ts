import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { formatErrorMessage } from './errorMessage'
import { toAppError } from './types'

/**
 * 翻译错误码：优先查插件命名空间下的 errors.<code>，再查 common，最后回退到错误码本身。
 */
export function useErrorMessage(ns?: string) {
  const { t, i18n } = useTranslation(ns ? [ns, 'common'] : 'common')
  return useCallback(
    (error: unknown) => formatErrorMessage(toAppError(error), t, i18n, ns),
    [ns, t, i18n],
  )
}
