import { useState } from 'react'
import { Badge, Empty, Input, NumberInput, Panel, Select, Spinner, Switch, cn, toast } from '@toolforge/ui'
import { CopyButton, ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Binary, Hash, Layers } from 'lucide-react'
import { INPUT_BASES, type Converted } from './types'

const EXAMPLES = ['255', '0xDEADBEEF', '-0b1010', '0o755', '18446744073709551615']

export function BaseConverter() {
  const { t, call, errorMessage } = usePlugin()
  const [input, setInput] = useState('0xDEADBEEF')
  const [from, setFrom] = useState(0)
  const [custom, setCustom] = useState(36)
  const [uppercase, setUppercase] = useState(true)
  const [group, setGroup] = useState(true)

  const trimmed = input.trim()
  const args = trimmed ? { input: trimmed, from, custom, uppercase, group } : null
  const { result, error, pending } = useDebouncedCall<Converted>(args ? 'convert' : null, args, [trimmed, from, custom, uppercase, group])
  const value = error ? null : result

  const toggle = async (bit: number) => {
    if (!args) return
    try {
      const next = await call<Converted>('convert', { ...args, toggle: bit })
      setInput(next.source)
      setFrom(next.base)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const baseLabel = (base: number) => (base === 0 ? t('input.auto') : t('input.base', { base }))

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="space-y-3 rounded-card border border-border bg-surface p-4 shadow-card">
        <div className="flex flex-wrap items-center gap-3">
          <Input
            value={input}
            onChange={(event) => setInput(event.target.value)}
            size="lg"
            className="font-mono"
            wrapperClassName="min-w-64 flex-1"
            placeholder={t('input.placeholder')}
            aria-label={t('input.label')}
            invalid={!!error}
            trailing={pending && <Spinner className="size-3.5" />}
          />
          <Select<number>
            value={from}
            onValueChange={setFrom}
            className="w-40"
            aria-label={t('input.from')}
            options={INPUT_BASES.map((base) => ({ value: base, label: baseLabel(base) }))}
          />
        </div>
        <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={uppercase} onCheckedChange={setUppercase} aria-label={t('options.uppercase')} />
            {t('options.uppercase')}
          </label>
          <label className="flex items-center gap-2 text-[13px] text-fg">
            <Switch size="sm" checked={group} onCheckedChange={setGroup} aria-label={t('options.group')} />
            {t('options.group')}
          </label>
          <div className="ml-auto flex flex-wrap items-center gap-1.5">
            <span className="text-xs text-fg-subtle">{t('input.examples')}</span>
            {EXAMPLES.map((example) => (
              <button
                key={example}
                type="button"
                onClick={() => {
                  setInput(example)
                  setFrom(0)
                }}
                className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
              >
                {example}
              </button>
            ))}
          </div>
        </div>
        {error && (
          <p className="text-xs text-danger">{errorMessage(error)}</p>
        )}
      </section>

      <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] gap-3">
        <Panel
          icon={<Hash />}
          title={t('results.title')}
          extra={value && <Badge>{t('results.size', { bits: value.bitLength, bytes: value.byteLength })}</Badge>}
          bodyClassName="overflow-auto py-1"
        >
          {!value ? (
            <Empty icon={<Hash />} title={t('results.empty')} />
          ) : (
            <>
              <ValueRow label={t('results.binary')} value={value.binary} />
              <ValueRow label={t('results.octal')} value={value.octal} />
              <ValueRow label={t('results.decimal')} value={value.decimal} />
              <ValueRow label={t('results.hex')} value={value.hex} />
              <div className="mx-3 my-1 border-t border-border" />
              <div className="flex items-center gap-2 px-3 pt-1.5">
                <span className="text-xs font-semibold text-fg-muted">{t('results.custom')}</span>
                <NumberInput size="sm" value={custom} onValueChange={setCustom} min={2} max={36} className="w-28" aria-label={t('results.custom')} />
              </div>
              <ValueRow label={t('input.base', { base: value.customBase })} value={value.custom} />
            </>
          )}
        </Panel>

        <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
          <Panel icon={<Binary />} title={t('bits.title')} extra={<span className="text-xs text-fg-subtle">{t('bits.hint')}</span>} bodyClassName="p-3">
            {value?.bits ? (
              <BitGrid bits={value.bits} onToggle={toggle} label={(bit) => t('bits.toggle', { bit })} />
            ) : (
              <p className="py-3 text-center text-xs text-fg-subtle">{value ? t('bits.outOfRange') : t('results.empty')}</p>
            )}
          </Panel>
          <Panel icon={<Layers />} title={t('widths.title')} bodyClassName="overflow-auto">
            {!value || value.widths.length === 0 ? (
              <Empty icon={<Layers />} title={value ? t('widths.none') : t('results.empty')} />
            ) : (
              <table className="w-full text-left text-xs">
                <thead className="sticky top-0 bg-surface-2 text-fg-muted">
                  <tr>
                    <th className="px-3 py-2 font-medium">{t('widths.bits')}</th>
                    <th className="px-3 py-2 font-medium">{t('widths.hex')}</th>
                    <th className="px-3 py-2 font-medium">{t('widths.unsigned')}</th>
                    <th className="px-3 py-2 font-medium">{t('widths.signed')}</th>
                  </tr>
                </thead>
                <tbody className="font-mono text-fg">
                  {value.widths.map((width) => (
                    <tr key={width.bits} className="group border-t border-border hover:bg-hover">
                      <td className="px-3 py-2 font-sans font-medium text-fg-muted">{t('widths.width', { bits: width.bits })}</td>
                      <td className="px-3 py-2 break-all" data-selectable>
                        <span className="inline-flex items-center gap-1">
                          {width.hex}
                          <CopyButton text={width.hex} className="opacity-0 group-hover:opacity-100" />
                        </span>
                      </td>
                      <td className="px-3 py-2 break-all" data-selectable>
                        {width.unsigned}
                      </td>
                      <td className="px-3 py-2 break-all" data-selectable>
                        {width.signed}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Panel>
        </div>
      </div>
    </div>
  )
}

/** 64 位视图：每行 16 位，点击任一位由后端翻转 */
function BitGrid({ bits, onToggle, label }: { bits: string; onToggle: (bit: number) => void; label: (bit: number) => string }) {
  const rows = [0, 1, 2, 3].map((row) => bits.slice(row * 16, row * 16 + 16))
  return (
    <div className="space-y-1.5">
      {rows.map((row, rowIndex) => (
        <div key={rowIndex} className="flex items-center gap-2">
          <span className="w-6 shrink-0 text-right font-mono text-[10px] text-fg-subtle tabular-nums">{63 - rowIndex * 16}</span>
          <div className="grid flex-1 grid-cols-4 gap-2">
            {[0, 1, 2, 3].map((nibble) => (
              <div key={nibble} className="grid grid-cols-4 gap-0.5">
                {row
                  .slice(nibble * 4, nibble * 4 + 4)
                  .split('')
                  .map((digit, offset) => {
                    const bit = 63 - (rowIndex * 16 + nibble * 4 + offset)
                    return (
                      <button
                        key={bit}
                        type="button"
                        aria-label={label(bit)}
                        title={label(bit)}
                        onClick={() => onToggle(bit)}
                        className={cn(
                          'h-6 rounded-sm font-mono text-[11px] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                          digit === '1' ? 'bg-primary text-fg-on-primary hover:bg-primary-hover' : 'bg-surface-2 text-fg-subtle hover:bg-hover',
                        )}
                      >
                        {digit}
                      </button>
                    )
                  })}
              </div>
            ))}
          </div>
          <span className="w-6 shrink-0 font-mono text-[10px] text-fg-subtle tabular-nums">{48 - rowIndex * 16}</span>
        </div>
      ))}
    </div>
  )
}
