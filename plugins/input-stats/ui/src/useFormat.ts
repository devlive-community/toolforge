import { useMemo } from 'react'
import { usePlugin } from '@toolforge/plugin-ui-sdk'

/** 按当前语言格式化数字、时长与距离 */
export function useFormat() {
  const { t } = usePlugin()
  return useMemo(() => {
    const integer = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 })
    const decimal = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 })
    const percent = new Intl.NumberFormat(undefined, { style: 'percent', maximumFractionDigits: 1 })
    return {
      count: (n: number) => integer.format(n),
      decimal: (n: number) => decimal.format(n),
      percent: (n: number) => percent.format(n),
      duration: (seconds: number) => {
        const h = Math.floor(seconds / 3600)
        const m = Math.floor((seconds % 3600) / 60)
        if (h > 0) return t('unit.hours', { h, m })
        if (m > 0) return t('unit.minutes', { m })
        return t('unit.seconds', { s: Math.floor(seconds) })
      },
      distance: (mm: number) => {
        if (mm >= 1_000_000) return t('unit.km', { value: decimal.format(mm / 1_000_000) })
        if (mm >= 1000) return t('unit.m', { value: decimal.format(mm / 1000) })
        return t('unit.cm', { value: decimal.format(mm / 10) })
      },
    }
  }, [t])
}
