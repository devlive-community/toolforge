import { useState, type ReactNode } from 'react'
import { Button, CodeEditor, Empty, Input, NumberInput, Panel, SegmentedControl, Spinner, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download, FileText, QrCode, SlidersHorizontal } from 'lucide-react'
import { ECC_LEVELS, type Ecc, type Generated } from './types'

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
    </div>
  )
}

export function Generate() {
  const { t, call, errorMessage } = usePlugin()
  const [text, setText] = useState('https://github.com/devlive-community/toolforge')
  const [ecc, setEcc] = useState<Ecc>('M')
  const [scale, setScale] = useState(8)
  const [margin, setMargin] = useState(4)
  const [dark, setDark] = useState('#000000') // tf-allow: 二维码默认前景色输入值
  const [light, setLight] = useState('#ffffff') // tf-allow: 二维码默认背景色输入值

  const options = { text, ecc, scale, margin, dark, light }
  const { result, error, pending } = useDebouncedCall<Generated>(text ? 'generate' : null, text ? options : null, [text, ecc, scale, margin, dark, light])
  const code = error ? null : result
  const colorError = error?.code === 'qr.invalid_color' ? (error.params?.field as string) : null

  const save = async (format: 'png' | 'svg') => {
    try {
      const path = await host.dialog.saveFile(`qrcode.${format}`, [{ name: format.toUpperCase(), extensions: [format] }])
      if (!path) return
      await call('save', { ...options, format, path })
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(320px,0.9fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel icon={<FileText />} title={t('generate.content')} footer={code && <span>{t('generate.bytes', { count: code.bytes })}</span>}>
          <CodeEditor value={text} onChange={setText} language="text" lineWrapping placeholder={t('generate.placeholder')} aria-label={t('generate.content')} />
        </Panel>
        <Panel icon={<SlidersHorizontal />} title={t('options.title')} className="shrink-0" bodyClassName="grid grid-cols-2 gap-3 p-3">
          <Field label={t('options.ecc')}>
            <SegmentedControl<Ecc>
              size="sm"
              value={ecc}
              onValueChange={setEcc}
              aria-label={t('options.ecc')}
              options={ECC_LEVELS.map((level) => ({ value: level, label: t(`ecc.${level}`) }))}
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label={t('options.scale')}>
              <NumberInput size="sm" value={scale} onValueChange={setScale} min={1} max={64} aria-label={t('options.scale')} />
            </Field>
            <Field label={t('options.margin')}>
              <NumberInput size="sm" value={margin} onValueChange={setMargin} min={0} max={16} aria-label={t('options.margin')} />
            </Field>
          </div>
          <Field label={t('options.dark')}>
            <ColorInput value={dark} onChange={setDark} invalid={colorError === 'dark'} label={t('options.dark')} />
          </Field>
          <Field label={t('options.light')}>
            <ColorInput value={light} onChange={setLight} invalid={colorError === 'light'} label={t('options.light')} />
          </Field>
        </Panel>
      </div>

      <Panel
        icon={<QrCode />}
        title={t('generate.preview')}
        extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
        actions={
          <>
            <Button size="sm" variant="outline" disabled={!code} onClick={() => save('svg')}>
              <Download />
              {t('generate.saveSvg')}
            </Button>
            <Button size="sm" variant="primary" disabled={!code} onClick={() => save('png')}>
              <Download />
              {t('generate.savePng')}
            </Button>
          </>
        }
        footer={
          code && (
            <>
              <span>{t('generate.version', { version: code.version, modules: code.modules })}</span>
              <span className="ml-auto">{t('generate.pixels', { size: code.pixels })}</span>
            </>
          )
        }
        bodyClassName="flex items-center justify-center overflow-auto p-6"
      >
        {error ? (
          <p className="text-center text-[13px] text-danger">{errorMessage(error)}</p>
        ) : code ? (
          <img
            src={code.image}
            alt={t('generate.preview')}
            className="aspect-square max-h-full w-full max-w-sm rounded-control border border-border object-contain [image-rendering:pixelated]"
          />
        ) : (
          <Empty icon={<QrCode />} title={t('generate.empty')} />
        )}
      </Panel>
    </div>
  )
}

function ColorInput({ value, onChange, invalid, label }: { value: string; onChange: (value: string) => void; invalid: boolean; label: string }) {
  return (
    <Input
      size="sm"
      value={value}
      onChange={(event) => onChange(event.target.value)}
      invalid={invalid}
      className="font-mono"
      aria-label={label}
      leading={<span className="size-3.5 rounded-sm border border-border" style={{ backgroundColor: invalid ? undefined : value }} />}
    />
  )
}
