import { useState } from 'react'
import { Badge, Button, Checkbox, CodeEditor, Panel, Select, Spinner, Switch, Tooltip, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowRight, ArrowRightLeft, ClipboardPaste, Download, FileText, FolderOpen, Terminal, Wand2 } from 'lucide-react'
import { EXTENSIONS, FORMATS, SAMPLES, type Concrete, type ConvertResult, type Delimiter, type Format, type Options } from './types'

export function ConverterTool() {
  const { t, errorMessage } = usePlugin()
  const [from, setFrom] = useState<Format>('auto')
  const [to, setTo] = useState<Concrete>('yaml')
  const [input, setInput] = useState(SAMPLES.json)
  const [options, setOptions] = useState<Options>({ indent: 2, minify: false, delimiter: 'comma', header: true, inferTypes: true })
  const set = (patch: Partial<Options>) => setOptions((prev) => ({ ...prev, ...patch }))

  const has = input.trim().length > 0
  const { result, error, pending } = useDebouncedCall<ConvertResult>(
    has ? 'convert' : null,
    has ? { input, from, to, options } : null,
    [input, from, to, ...Object.values(options)],
  )
  const detected = result?.from ?? (from === 'auto' ? null : from)
  const errorLine = error ? Number(error.params?.line ?? 0) || null : null
  const csvInvolved = detected === 'csv' || to === 'csv'

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action()
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const swap = () => {
    if (!result || error) return
    setInput(result.output)
    setFrom(to)
    setTo(result.from)
  }

  const formatOptions = FORMATS.map((value) => ({ value, label: t(`formats.${value}`) }))

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex shrink-0 flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <Select<Format>
          value={from}
          onValueChange={setFrom}
          className="w-36"
          aria-label={t('from')}
          options={[{ value: 'auto', label: t('formats.auto') }, ...formatOptions]}
        />
        <ArrowRight className="size-4 text-fg-subtle" />
        <Select<Concrete> value={to} onValueChange={setTo} className="w-36" aria-label={t('to')} options={formatOptions} />
        <Tooltip content={t('swap')}>
          <Button size="icon-md" aria-label={t('swap')} onClick={swap} disabled={!result || !!error}>
            <ArrowRightLeft />
          </Button>
        </Tooltip>

        <div className="flex flex-wrap items-center gap-3 border-l border-border pl-3 text-[13px] text-fg-muted">
          {to === 'json' && (
            <>
              <Select<string>
                size="sm"
                value={String(options.indent)}
                onValueChange={(value) => set({ indent: Number(value) })}
                aria-label={t('options.indent')}
                options={[
                  { value: '2', label: t('options.indent2') },
                  { value: '4', label: t('options.indent4') },
                ]}
              />
              <label className="flex items-center gap-2">
                {t('options.minify')}
                <Switch size="sm" checked={options.minify} onCheckedChange={(minify) => set({ minify })} aria-label={t('options.minify')} />
              </label>
            </>
          )}
          {csvInvolved && (
            <Select<Delimiter>
              size="sm"
              value={options.delimiter}
              onValueChange={(delimiter) => set({ delimiter })}
              aria-label={t('options.delimiter')}
              options={(['comma', 'semicolon', 'tab', 'pipe'] as Delimiter[]).map((value) => ({ value, label: t(`delimiters.${value}`) }))}
            />
          )}
          {detected === 'csv' && (
            <>
              <Checkbox checked={options.header} onCheckedChange={(header) => set({ header })}>
                {t('options.header')}
              </Checkbox>
              <Checkbox checked={options.inferTypes} onCheckedChange={(inferTypes) => set({ inferTypes })}>
                {t('options.inferTypes')}
              </Checkbox>
            </>
          )}
        </div>
      </section>

      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel
          icon={<FileText />}
          title={t('input')}
          extra={
            <Button variant="soft" size="sm" className="ml-1" onClick={() => setInput(SAMPLES[from === 'auto' ? 'json' : from])}>
              <Wand2 />
              {t('sample')}
            </Button>
          }
          actions={
            <>
              <Button size="sm" onClick={() => setInput('')}>
                {t('common:clear')}
              </Button>
              <Button size="sm" onClick={() => run(async () => setInput(await host.clipboard.readText()))}>
                <ClipboardPaste />
                {t('common:paste')}
              </Button>
              <Tooltip content={t('open')}>
                <Button
                  size="icon-sm"
                  aria-label={t('open')}
                  onClick={() =>
                    run(async () => {
                      const path = await host.dialog.openFile([{ name: 'JSON / YAML / TOML / CSV', extensions: FORMATS.flatMap((f) => EXTENSIONS[f]) }])
                      if (path) setInput(await host.fs.readText(path))
                    })
                  }
                >
                  <FolderOpen />
                </Button>
              </Tooltip>
            </>
          }
          footer={
            detected && (
              <Badge variant="info">{from === 'auto' ? t('detected', { format: t(`formats.${detected}`) }) : t(`formats.${detected}`)}</Badge>
            )
          }
        >
          <CodeEditor value={input} onChange={setInput} language={detected === 'json' ? 'json' : 'text'} errorLine={errorLine} placeholder={t('placeholder')} aria-label={t('input')} />
        </Panel>

        <Panel
          icon={<Terminal />}
          title={t('output', { format: t(`formats.${to}`) })}
          extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            <>
              <CopyButton text={result?.output ?? ''} label={t('common:copy')} variant="outline" disabled={!result || !!error} />
              <Button
                size="sm"
                disabled={!result || !!error}
                onClick={() =>
                  run(async () => {
                    if (!result) return
                    const path = await host.dialog.saveFile(`converted.${EXTENSIONS[to][0]}`, [{ name: t(`formats.${to}`), extensions: EXTENSIONS[to] }])
                    if (!path) return
                    await host.fs.writeText(path, result.output)
                    toast.success(t('common:saved'))
                  })
                }
              >
                <Download />
                {t('common:download')}
              </Button>
            </>
          }
          footer={
            result &&
            !error && (
              <>
                {result.records !== null && <span>{t('records', { count: result.records })}</span>}
                <span className="ml-auto">{t('elapsed', { ms: result.elapsedMs })}</span>
              </>
            )
          }
        >
          {!has ? (
            <p className="p-6 text-center text-[13px] text-fg-subtle">{t('waiting')}</p>
          ) : error ? (
            <div className="p-4 text-[13px]">
              <p className="text-danger">{errorMessage(error)}</p>
              {typeof error.params?.detail === 'string' && (
                <p className="mt-2 rounded-control bg-surface-2 px-2.5 py-1.5 font-mono text-xs break-all text-fg-muted" data-selectable>
                  {error.params.detail}
                </p>
              )}
            </div>
          ) : (
            <CodeEditor value={result?.output ?? ''} readOnly language={to === 'json' ? 'json' : 'text'} aria-label={t('output', { format: to })} />
          )}
        </Panel>
      </div>
    </div>
  )
}
