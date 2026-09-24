import { useState } from 'react'
import { Badge, Empty, Input, Panel, Spinner, Tooltip, cn } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Blend, Contrast as ContrastIcon, Palette, Pipette } from 'lucide-react'
import type { Contrast, Converted } from './types'

const EXAMPLES = ['#3b82f6', 'rebeccapurple', 'hsl(142 71% 45%)', 'oklch(0.7 0.15 30)', 'rgba(255, 99, 71, 0.6)'] // tf-allow: 示例输入

export function ColorTool() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('#16a34a') // tf-allow: 默认输入值
  const [compare, setCompare] = useState('#ffffff') // tf-allow: 默认对比色

  const trimmed = input.trim()
  const args = trimmed ? { input: trimmed, compare } : null
  const { result, error, pending } = useDebouncedCall<Converted>(args ? 'convert' : null, args, [trimmed, compare])
  const color = error ? null : result
  const compareError = error?.params?.field === 'compare'

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.3fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3 overflow-auto">
        <section className="overflow-hidden rounded-card border border-border bg-surface shadow-card">
          <div
            className="flex h-36 flex-col justify-end p-4 transition-colors"
            style={color ? { backgroundColor: color.hex, color: color.text } : undefined}
          >
            {color ? (
              <>
                <p className="font-mono text-2xl font-semibold tracking-tight">{color.hex.toUpperCase()}</p>
                <p className="text-xs opacity-80">
                  {color.name ?? t('swatch.unnamed')}
                  {color.alpha < 1 && ` · ${t('swatch.alpha', { value: color.alpha })}`}
                </p>
              </>
            ) : (
              <p className="m-auto text-xs text-fg-subtle">{t('swatch.empty')}</p>
            )}
          </div>
          <div className="space-y-2 p-3">
            <Input
              value={input}
              onChange={(event) => setInput(event.target.value)}
              className="font-mono"
              leading={<Pipette />}
              trailing={pending && <Spinner className="size-3.5" />}
              invalid={!!error && !compareError}
              placeholder={t('input.placeholder')}
              aria-label={t('input.label')}
            />
            {error && !compareError && <p className="text-xs text-danger">{errorMessage(error)}</p>}
            <div className="flex flex-wrap gap-1.5">
              {EXAMPLES.map((example) => (
                <button
                  key={example}
                  type="button"
                  onClick={() => setInput(example)}
                  className="rounded-control border border-border px-2 py-0.5 font-mono text-[11px] text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {example}
                </button>
              ))}
            </div>
          </div>
        </section>

        <Panel icon={<ContrastIcon />} title={t('contrast.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          {color ? (
            <>
              <ContrastRow label={t('contrast.onWhite')} contrast={color.onWhite} />
              <ContrastRow label={t('contrast.onBlack')} contrast={color.onBlack} />
              <div className="space-y-1.5 border-t border-border pt-2.5">
                <Input
                  size="sm"
                  value={compare}
                  onChange={(event) => setCompare(event.target.value)}
                  className="font-mono"
                  leading={
                    color.compareHex ? <span className="size-3.5 rounded-sm border border-border" style={{ backgroundColor: color.compareHex }} /> : <Blend />
                  }
                  invalid={compareError}
                  placeholder={t('contrast.comparePlaceholder')}
                  aria-label={t('contrast.compare')}
                />
                {compareError && error && <p className="text-xs text-danger">{errorMessage(error)}</p>}
                {color.compare && <ContrastRow label={t('contrast.compare')} contrast={color.compare} />}
              </div>
            </>
          ) : (
            <p className="py-2 text-center text-xs text-fg-subtle">{t('swatch.empty')}</p>
          )}
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel icon={<Palette />} title={t('formats.title')} bodyClassName="overflow-auto py-1">
          {color ? (
            color.formats.map((format) => <ValueRow key={format.key} label={t(`formats.${format.key}`)} value={format.value} labelWidth="w-24" />)
          ) : (
            <Empty icon={<Palette />} title={t('swatch.empty')} />
          )}
        </Panel>
        <Panel icon={<Blend />} title={t('palette.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
          {color ? (
            <>
              <SwatchRow label={t('palette.tints')} colors={color.tints} onPick={setInput} />
              <SwatchRow label={t('palette.shades')} colors={color.shades} onPick={setInput} />
              <div className="grid grid-cols-2 gap-3">
                {color.harmonies.map((harmony) => (
                  <SwatchRow key={harmony.key} label={t(`palette.${harmony.key}`)} colors={harmony.colors} onPick={setInput} />
                ))}
              </div>
            </>
          ) : (
            <p className="py-2 text-center text-xs text-fg-subtle">{t('swatch.empty')}</p>
          )}
        </Panel>
      </div>
    </div>
  )
}

function ContrastRow({ label, contrast }: { label: string; contrast: Contrast }) {
  const { t } = usePlugin()
  const levels: [string, boolean][] = [
    ['AA', contrast.aa],
    ['AAA', contrast.aaa],
    [t('contrast.large', { level: 'AA' }), contrast.aaLarge],
    [t('contrast.large', { level: 'AAA' }), contrast.aaaLarge],
  ]
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="w-20 shrink-0 text-xs text-fg-muted">{label}</span>
      <span className="w-14 font-mono text-[13px] font-semibold text-fg tabular-nums">{t('contrast.ratio', { ratio: contrast.ratio })}</span>
      <div className="flex flex-wrap gap-1">
        {levels.map(([name, pass]) => (
          <Badge key={name} variant={pass ? 'success' : 'neutral'} className={cn(!pass && 'line-through opacity-60')}>
            {name}
          </Badge>
        ))}
      </div>
    </div>
  )
}

function SwatchRow({ label, colors, onPick }: { label: string; colors: string[]; onPick: (hex: string) => void }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      <div className="flex overflow-hidden rounded-control border border-border">
        {colors.map((hex, index) => (
          <Tooltip key={`${hex}-${index}`} content={hex}>
            <button
              type="button"
              aria-label={hex}
              onClick={() => onPick(hex)}
              className="h-8 flex-1 outline-none transition-transform hover:z-10 hover:scale-y-110 focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring"
              style={{ backgroundColor: hex }}
            />
          </Tooltip>
        ))}
      </div>
    </div>
  )
}
