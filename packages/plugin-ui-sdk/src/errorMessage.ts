import type { i18n, TFunction } from 'i18next'
import type { AppError } from './types'

function valueAt(params: Record<string, unknown>, name: string): unknown {
  const nested = name.split('.').reduce<unknown>((value, key) => (
    value !== null && typeof value === 'object' && Object.hasOwn(value, key)
      ? (value as Record<string, unknown>)[key]
      : undefined
  ), params)
  return nested === undefined && Object.hasOwn(params, name) ? params[name] : nested
}

/** 检查翻译原文，不能在插值后把用户详情中的 {{...}} 当成缺失参数。 */
function missingParameter(template: string, params: Record<string, unknown>, defaults: Record<string, unknown>): boolean {
  for (const match of template.matchAll(/\{\{\s*-?\s*([^{}]+?)\s*\}\}/g)) {
    const name = match[1].split(',')[0].trim()
    if (valueAt(params, name) === undefined && valueAt(defaults, name) === undefined) return true
  }
  return false
}

/** 同时用于插件、实时任务和历史任务；旧记录缺少参数时给出可读的降级提示。 */
export function formatErrorMessage(error: AppError, t: TFunction, instance: i18n, ns?: string): string {
  const { code, params } = error
  const key = `errors.${code}`
  const namespaces = ns ? [ns, 'common'] : ['common']
  if (!instance.exists(key, { ...params, ns: namespaces })) return code

  const translated = t(key, {
    ...params,
    ns: namespaces,
    returnDetails: true,
    interpolation: { skipOnVariables: true },
  })
  const template: unknown = instance.getResource(translated.usedLng, translated.usedNS, translated.exactUsedKey)
  const defaults = instance.options.interpolation?.defaultVariables ?? {}
  if (typeof template === 'string' && missingParameter(template, params ?? {}, defaults)) {
    return t('errors.task.details_missing', { ns: 'common', code })
  }
  return typeof translated.res === 'string' ? translated.res : code
}
