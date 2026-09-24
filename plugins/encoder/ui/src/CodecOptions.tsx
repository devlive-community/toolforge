import { Select, Switch } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import type { ReactNode } from 'react'
import type { Codec, Direction, HtmlMode, Options, UnicodeStyle } from './types'

function Toggle({ label, checked, onChange }: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-[13px] text-fg-muted">
      {label}
      <Switch size="sm" checked={checked} onCheckedChange={onChange} aria-label={label} />
    </label>
  )
}

interface Props {
  codec: Codec
  direction: Direction
  options: Options
  onChange: (patch: Partial<Options>) => void
}

/** 当前编码类型在编码方向上的可选项 */
export function CodecOptions({ codec, direction, options, onChange }: Props) {
  const { t } = usePlugin()
  if (direction === 'decode') return null
  const items: ReactNode[] = []
  if (codec === 'base64' || codec === 'base64url' || codec === 'base32') {
    items.push(<Toggle key="padding" label={t('options.padding')} checked={options.padding} onChange={(padding) => onChange({ padding })} />)
  }
  if (codec === 'base64') {
    items.push(<Toggle key="wrap" label={t('options.wrap')} checked={options.wrap} onChange={(wrap) => onChange({ wrap })} />)
  }
  if (codec === 'hex') {
    items.push(<Toggle key="upper" label={t('options.uppercase')} checked={options.uppercase} onChange={(uppercase) => onChange({ uppercase })} />)
  }
  if (codec === 'html') {
    items.push(
      <Select<HtmlMode>
        key="html"
        size="sm"
        value={options.htmlMode}
        onValueChange={(htmlMode) => onChange({ htmlMode })}
        aria-label={t('options.htmlMode')}
        options={[
          { value: 'basic', label: t('options.htmlBasic') },
          { value: 'decimal', label: t('options.htmlDecimal') },
          { value: 'hex', label: t('options.htmlHex') },
        ]}
      />,
    )
  }
  if (codec === 'unicode') {
    items.push(
      <Select<UnicodeStyle>
        key="style"
        size="sm"
        value={options.unicodeStyle}
        onValueChange={(unicodeStyle) => onChange({ unicodeStyle })}
        aria-label={t('options.unicodeStyle')}
        options={[
          { value: 'u', label: '\\uXXXX' },
          { value: 'braces', label: '\\u{X}' },
          { value: 'codepoint', label: 'U+XXXX' },
        ]}
      />,
    )
    if (options.unicodeStyle !== 'codepoint') {
      items.push(
        <Toggle key="ascii" label={t('options.escapeAscii')} checked={options.escapeAscii} onChange={(escapeAscii) => onChange({ escapeAscii })} />,
      )
    }
  }
  return <>{items}</>
}
