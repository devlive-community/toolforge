import type { ReactNode } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, NumberInput, Panel, Select, Spinner, Switch, toast } from '@toolforge/ui'
import { host, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Barcode, Download, FileText, SlidersHorizontal } from 'lucide-react'
import { FORMATS, MATRIX_FORMATS, SAMPLES, type Format, type Generated } from './types'

function Field({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-1.5">
      <div className="text-xs font-medium text-fg-muted">{label}</div>
      {children}
    </div>
  )
}

interface GenerateProps {
  format: Format
  setFormat: (format: Format) => void
  text: string
  setText: (text: string) => void
}

export function Generate({ format, setFormat, text, setText }: GenerateProps) {
  const { t, call, errorMessage } = usePlugin()
  const [height, setHeight] = usePluginState('height', 60)
  const [scale, setScale] = usePluginState('scale', 3)
  const [showText, setShowText] = usePluginState('showText', true)
  const [dark, setDark] = usePluginState('dark', '#000000') // tf-allow: 条码默认前景色输入值
  const [light, setLight] = usePluginState('light', '#ffffff') // tf-allow: 条码默认背景色输入值
  const matrix = MATRIX_FORMATS.has(format)

  const changeFormat = (next: Format) => {
    // 内容还是上一个码制的示例（或为空）时，换成新码制的示例
    if (!text.trim() || text === SAMPLES[format]) setText(SAMPLES[next])
    setFormat(next)
  }

  const options = { format, text, height, scale, showText, dark, light }
  const { result, error, pending } = useDebouncedCall<Generated>(text ? 'generate' : null, text ? options : null, [
    format,
    text,
    height,
    scale,
    showText,
    dark,
    light,
  ])
  const code = error ? null : result
  const colorError = error?.code === 'barcode.invalid_color' ? (error.params?.field as string) : null

  const save = async (saveAs: 'png' | 'svg') => {
    try {
      const path = await host.dialog.saveFile(`${format}-${(code?.content ?? 'barcode').replace(/[^\w-]+/g, '_').slice(0, 40)}.${saveAs}`, [
        { name: saveAs.toUpperCase(), extensions: [saveAs] },
      ])
      if (!path) return
      await call('save', { ...options, saveAs, path })
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Panel
          icon={<FileText />}
          title={t('generate.content')}
          className={matrix ? 'min-h-0 flex-1' : 'shrink-0'}
          bodyClassName={matrix ? 'flex min-h-0 flex-col gap-3 p-3' : 'space-y-3 p-3'}
        >
          <Field label={t('generate.format')}>
            <Select<Format>
              value={format}
              onValueChange={changeFormat}
              options={FORMATS.map((f) => ({ value: f, label: t(`formats.${f}`) }))}
              className="w-full"
              aria-label={t('generate.format')}
            />
            <p className="text-[11px] leading-relaxed text-fg-subtle">{t(`hints.${format}`)}</p>
          </Field>
          {matrix ? (
            <div className="min-h-32 flex-1 overflow-hidden rounded-control border border-border">
              <CodeEditor
                value={text}
                onChange={setText}
                language="text"
                lineWrapping
                placeholder={t('generate.placeholder')}
                aria-label={t('generate.content')}
              />
            </div>
          ) : (
            <Input
              value={text}
              onChange={(e) => setText(e.target.value)}
              className="font-mono"
              placeholder={t('generate.placeholder')}
              aria-label={t('generate.content')}
            />
          )}
          {code && (code.checkDigit || code.notes.length > 0) && (
            <div className="flex flex-wrap gap-1.5">
              {code.checkDigit && <Badge variant="success">{t('generate.checkDigit', { digit: code.checkDigit, content: code.content })}</Badge>}
              {code.notes.map((note) => (
                <Badge key={note} variant="warning">
                  {t(`notes.${note}`)}
                </Badge>
              ))}
            </div>
          )}
        </Panel>
        <Panel icon={<SlidersHorizontal />} title={t('options.title')} className="shrink-0" bodyClassName="grid grid-cols-2 gap-3 p-3">
          {!matrix && (
            <Field label={t('options.height')}>
              <NumberInput size="sm" value={height} onValueChange={setHeight} min={10} max={300} step={5} aria-label={t('options.height')} />
            </Field>
          )}
          <Field label={t('options.scale')}>
            <NumberInput size="sm" value={scale} onValueChange={setScale} min={1} max={20} aria-label={t('options.scale')} />
          </Field>
          <Field label={t('options.dark')}>
            <ColorInput value={dark} onChange={setDark} invalid={colorError === 'dark'} label={t('options.dark')} />
          </Field>
          <Field label={t('options.light')}>
            <ColorInput value={light} onChange={setLight} invalid={colorError === 'light'} label={t('options.light')} />
          </Field>
          {!matrix && (
            <label className="col-span-2 flex items-center justify-between text-[13px] text-fg">
              {t('options.showText')}
              <Switch size="sm" checked={showText} onCheckedChange={setShowText} aria-label={t('options.showText')} />
            </label>
          )}
        </Panel>
      </div>

      <Panel
        icon={<Barcode />}
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
        footer={code && <span>{t('generate.pixels', { width: code.width, height: code.height })}</span>}
        bodyClassName="flex items-center justify-center overflow-auto bg-surface-2 p-6"
      >
        {error ? (
          <p className="text-center text-[13px] text-danger">{errorMessage(error)}</p>
        ) : code ? (
          <img
            src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(code.svg)}`}
            alt={t('generate.preview')}
            className="max-h-full max-w-full rounded-control border border-border object-contain shadow-card"
            style={{ width: code.width }}
          />
        ) : (
          <Empty icon={<Barcode />} title={t('generate.empty')} />
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
