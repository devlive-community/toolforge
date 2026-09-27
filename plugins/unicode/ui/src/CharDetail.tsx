import { Badge, Panel } from '@toolforge/ui'
import { CopyButton, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FLAG_TONE, type Found } from './types'

/** 看不见或会改变排版的字符在大字预览中用占位符显示 */
export function glyph(char: string, flags: string[]) {
  if (flags.includes('invisible') || flags.includes('bidi') || flags.includes('control')) return '⍰'
  if (char === ' ') return '␠'
  if (char === '\n') return '↵'
  if (char === '\t') return '⇥'
  return char
}

export function CharDetail({ found, onClose }: { found: Found; onClose?: () => void }) {
  const { t } = usePlugin()
  const rows: [string, string][] = [
    [t('detail.code'), found.code],
    [t('detail.decimal'), String(found.decimal)],
    ['UTF-8', found.utf8],
    ['UTF-16', found.utf16],
    ['Rust', found.escapes.rust],
    ['JavaScript', found.escapes.javascript],
    ['Python', found.escapes.python],
    ['Java / JSON', found.escapes.java],
    ['HTML', found.escapes.html],
    ['CSS', found.escapes.css],
    ['URL', found.escapes.url],
  ]
  return (
    <Panel
      title={found.name ?? found.code}
      actions={
        onClose && (
          <button type="button" className="rounded-sm px-1 text-xs text-fg-muted outline-none hover:text-fg focus-visible:ring-2 focus-visible:ring-ring" onClick={onClose}>
            {t('detail.close')}
          </button>
        )
      }
      bodyClassName="space-y-3 overflow-auto p-3"
    >
      <div className="flex items-center gap-4">
        <span className="flex size-20 items-center justify-center rounded-card border border-border bg-surface-2 text-5xl text-fg">{glyph(found.char, found.flags)}</span>
        <div className="min-w-0 space-y-1.5 text-xs text-fg-muted">
          <p>{t(`categories.${found.category}`, { defaultValue: found.category })}</p>
          <p>{t('detail.script', { script: found.script })}</p>
          <div className="flex flex-wrap gap-1">
            {found.flags.map((flag) => (
              <Badge key={flag} variant={FLAG_TONE[flag]}>
                {t(`flags.${flag}`)}
              </Badge>
            ))}
          </div>
        </div>
        <CopyButton text={found.char} label={t('detail.copyChar')} variant="outline" className="ml-auto" />
      </div>
      <dl className="divide-y divide-border">
        {rows.map(([label, value]) => (
          <div key={label} className="grid grid-cols-[100px_minmax(0,1fr)_auto] items-center gap-2 py-1.5">
            <dt className="text-xs text-fg-muted">{label}</dt>
            <dd className="font-mono text-[12px] break-all text-fg" data-selectable>
              {value}
            </dd>
            <CopyButton text={value} />
          </div>
        ))}
      </dl>
    </Panel>
  )
}
