import { useTranslation } from 'react-i18next'
import type { Manifest } from '@toolforge/plugin-ui-sdk'

/** 解析 manifest 中 `i18n:key` 形式的文案，在插件命名空间中翻译 */
export function usePluginText() {
  const { t } = useTranslation()
  return (manifest: Manifest, value: string) =>
    value.startsWith('i18n:') ? t(value.slice(5), { ns: manifest.id }) : value
}
