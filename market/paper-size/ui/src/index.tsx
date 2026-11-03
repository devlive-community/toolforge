import { useMemo, useState } from 'react'
import { Input, Panel, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileImage, ScrollText } from 'lucide-react'

interface SizeItem {
  key: string
  short: number
  long: number
  category: string
}

interface ListOut {
  sizes: SizeItem[]
  total: number
}

interface ConvertOut {
  mm: number
  inches: number
  pixels: number
  dpi: number
}

const DPIS = [72, 150, 300, 600]

export default function PaperSize() {
  const { t, errorMessage } = usePlugin()
  const [query, setQuery] = useState('')
  const [mm, setMm] = useState(210) // tf-allow: 默认值
  const [dpi, setDpi] = useState(300) // tf-allow: 默认值

  const needle = query.trim()
  const list = useDebouncedCall<ListOut>('list', { query: needle }, [needle])
  const convertArgs = mm > 0 && dpi > 0 ? { mm, dpi } : null
  const converted = useDebouncedCall<ConvertOut>(convertArgs ? 'convert' : null, convertArgs, [mm, dpi])

  const groups = useMemo(() => {
    const map = new Map<string, SizeItem[]>()
    for (const size of list.result?.sizes ?? []) {
      const bucket = map.get(size.category) ?? []
      bucket.push(size)
      map.set(size.category, bucket)
    }
    return [...map.entries()]
  }, [list.result])

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1.4fr)_minmax(300px,1fr)] gap-3">
      <Panel
        icon={<ScrollText />}
        title={t('api.list.summary')}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-3 p-3"
        extra={list.pending && <Spinner className="size-3.5" />}
      >
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          leading={<ScrollText />}
          placeholder={t('api.list.query')}
          aria-label={t('api.list.query')}
        />
        {list.error ? (
          <p className="text-xs text-danger">{errorMessage(list.error)}</p>
        ) : (
          groups.map(([category, sizes]) => (
            <section key={category} className="space-y-1">
              <p className="text-xs font-medium text-fg-muted">{t(`categories.${category}`)}</p>
              <table className="w-full text-xs">
                <thead>
                  <tr className="text-left text-fg-subtle">
                    <th className="py-1 font-medium">{t('table.key')}</th>
                    <th className="py-1 text-right font-medium">{t('table.short')}</th>
                    <th className="py-1 text-right font-medium">{t('table.long')}</th>
                    <th className="py-1 text-right font-medium">{t('table.inches')}</th>
                    <th className="py-1 text-right font-medium">{t('table.pixels', { dpi })}</th>
                  </tr>
                </thead>
                <tbody className="font-mono">
                  {sizes.map((size) => (
                    <tr key={size.key} className="border-t border-border">
                      <td className="py-1 font-sans">{t(`sizes.${size.key}`)}</td>
                      <td className="py-1 text-right tabular-nums">{size.short}</td>
                      <td className="py-1 text-right tabular-nums">{size.long}</td>
                      <td className="py-1 text-right tabular-nums">{(size.long / 25.4).toFixed(2)}</td>
                      <td className="py-1 text-right tabular-nums">{Math.round((size.short / 25.4) * dpi)}{'×'}{Math.round((size.long / 25.4) * dpi)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))
        )}
      </Panel>
      <Panel
        icon={<FileImage />}
        title={t('convert.title')}
        className="min-h-0 shrink-0"
        bodyClassName="space-y-2.5 p-3"
        extra={converted.pending && <Spinner className="size-3.5" />}
      >
        <Input
          type="number"
          value={mm === undefined ? '' : String(mm)}
          onChange={(event) => setMm(event.target.value === '' ? 0 : Number(event.target.value))}
          min={0}
          placeholder={t('convert.mmLabel')}
          aria-label={t('convert.mmLabel')}
        />
        <div className="flex gap-1.5">
          {DPIS.map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setDpi(value)}
              className={`rounded-control border px-2 py-0.5 font-mono text-[11px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring ${
                dpi === value ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:bg-hover hover:text-fg'
              }`}
            >
              {value} DPI
            </button>
          ))}
        </div>
        <Input
          type="number"
          value={dpi === undefined ? '' : String(dpi)}
          onChange={(event) => setDpi(event.target.value === '' ? 0 : Number(event.target.value))}
          min={0}
          placeholder={t('convert.dpiLabel')}
          aria-label={t('convert.dpiLabel')}
        />
        {converted.result ? (
          <div className="space-y-1 pt-1">
            <ValueRow label={t('convert.inches')} value={converted.result.inches.toFixed(3)} labelWidth="w-20" />
            <ValueRow label={t('convert.pixels')} value={String(converted.result.pixels)} labelWidth="w-20" />
          </div>
        ) : (
          <p className="text-xs text-fg-subtle">{t('convert.mmLabel')} / {t('convert.dpiLabel')}</p>
        )}
      </Panel>
    </div>
  )
}
