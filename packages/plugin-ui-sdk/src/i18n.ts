import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { toAppError } from './types'

/**
 * 翻译错误码：优先查插件命名空间下的 errors.<code>，再查 common，最后回退到错误码本身。
 */
export function useErrorMessage(ns?: string) {
  const { t, i18n } = useTranslation(ns ? [ns, 'common'] : 'common')
  return useCallback(
    (error: unknown) => {
      const { code, params } = toAppError(error)
      const key = `errors.${code}`
      for (const namespace of ns ? [ns, 'common'] : ['common']) {
        if (i18n.exists(key, { ns: namespace })) return t(key, { ns: namespace, ...params })
      }
      return code
    },
    [ns, t, i18n],
  )
}
