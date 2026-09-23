import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import zhCN from '../locales/zh-CN.json'
import enUS from '../locales/en-US.json'

export const SUPPORTED_LOCALES = ['zh-CN', 'en-US'] as const
export type Locale = (typeof SUPPORTED_LOCALES)[number]

export function resolveLocale(preferred?: string | null): Locale {
  const candidate = preferred ?? navigator.language
  if ((SUPPORTED_LOCALES as readonly string[]).includes(candidate)) return candidate as Locale
  return candidate.toLowerCase().startsWith('zh') ? 'zh-CN' : 'en-US'
}

export function initI18n(locale: Locale) {
  // 命名空间：app（宿主界面）、common（通用文案与错误码）、plugin.<id>（插件，加载时注入）
  return i18n.use(initReactI18next).init({
    lng: locale,
    fallbackLng: 'en-US',
    defaultNS: 'app',
    fallbackNS: 'common',
    ns: ['app', 'common'],
    resources: {
      'zh-CN': zhCN,
      'en-US': enUS,
    },
    interpolation: { escapeValue: false },
    returnNull: false,
  })
}

/** 注册插件自带的语言包，命名空间为插件 id */
export function addPluginResources(pluginId: string, bundles: Record<string, object>) {
  for (const [locale, resources] of Object.entries(bundles)) {
    i18n.addResourceBundle(locale, pluginId, resources, true, true)
  }
}

export { i18n }
