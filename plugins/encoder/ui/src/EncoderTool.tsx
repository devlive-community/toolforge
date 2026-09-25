import { useState } from 'react'
import { Button, CodeEditor, Panel, Progress, SegmentedControl, Spinner, Tooltip, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useCopy, useDebouncedCall, useLaunchInput, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { ArrowUpDown, Check, Copy, Download, FileText, FileUp, Info, Terminal, X } from 'lucide-react'
import { CodecOptions } from './CodecOptions'
import { GROUPS, type Codec, type Direction, type FileFormat, type FileResult, type Options, type TransformResult } from './types'

const BASE64_CODECS: Codec[] = ['base64']
/** 剪贴板识别的标签对应的解码方式 */
const LAUNCH_CODECS: Record<string, Codec> = { url: 'urlComponent', unicode: 'unicode', html: 'html', base64: 'base64', base64url: 'base64url' }

export function EncoderTool() {
  const { t, call, errorMessage } = usePlugin()
  const { copy, copied } = useCopy()
  const [codec, setCodec] = useState<Codec>('base64')
  const [direction, setDirection] = useState<Direction>('encode')
  const [input, setInput] = useState('ToolForge 工具箱 🔧 https://toolforge.dev/?q=a b&lang=zh')
  useLaunchInput((text, label) => {
    const launched = label ? LAUNCH_CODECS[label] : undefined
    if (launched) {
      setCodec(launched)
      setDirection('decode')
    }
    setInput(text)
  })
  const [options, setOptions] = useState<Options>({
    padding: true,
    wrap: false,
    uppercase: false,
    htmlMode: 'basic',
    unicodeStyle: 'u',
    escapeAscii: false,
  })
  const [fileFormat, setFileFormat] = useState<FileFormat>('dataUri')
  const [showFile, setShowFile] = useState(false)
  const task = useTask<FileResult>()

  const optionKey = Object.values(options).join('|')
  const hasInput = input.length > 0
  const { result, error, pending } = useDebouncedCall<TransformResult>(
    hasInput ? 'transform' : null,
    hasInput ? { input, codec, direction, options } : null,
    [input, codec, direction, optionKey],
  )

  const fileMode = showFile && task.status !== 'idle'
  const output = fileMode ? (task.result?.preview ?? '') : error ? '' : (result?.output ?? '')

  const swap = () => {
    if (!result || error || !result.text) return
    setInput(result.output)
    setDirection((d) => (d === 'encode' ? 'decode' : 'encode'))
  }

  const encodeFile = async () => {
    try {
      const path = await host.dialog.openFile()
      if (!path) return
      setShowFile(true)
      await task.start('encode_file', { path, format: fileFormat, wrap: false })
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const file = fileMode ? task.result : null

  const copyFile = async () => {
    if (!file) return
    try {
      const { output: full } = await call<{ output: string }>('file_output', { id: file.id })
      await copy(full)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const saveFile = async () => {
    if (!file) return
    try {
      const path = await host.dialog.saveFile(`${file.name}.${fileFormat === 'dataUri' ? 'datauri' : 'b64'}.txt`)
      if (!path) return
      await call('save_output', { id: file.id, path })
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const percent = task.progress && task.progress.total > 0 ? (task.progress.done / task.progress.total) * 100 : null

  return (
    <div className="grid h-full min-h-0 grid-cols-[200px_minmax(0,1fr)] gap-3">
      <nav className="flex min-h-0 flex-col gap-4 overflow-auto rounded-card border border-border bg-surface p-2 shadow-card">
        {GROUPS.map((group) => (
          <div key={group.key}>
            <p className="px-2 pt-1 pb-1.5 text-[11px] font-semibold tracking-wide text-fg-subtle uppercase">{t(`groups.${group.key}`)}</p>
            {group.codecs.map((item) => (
              <button
                key={item}
                type="button"
                aria-current={item === codec}
                onClick={() => {
                  setCodec(item)
                  setShowFile(false)
                }}
                className={cn(
                  'flex w-full flex-col rounded-control px-2.5 py-1.5 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                  item === codec ? 'bg-primary-soft text-primary-fg' : 'text-fg hover:bg-hover',
                )}
              >
                <span className="text-[13px] font-medium">{t(`codecs.${item}.name`)}</span>
                <span className={cn('text-[11px]', item === codec ? 'text-primary-fg/80' : 'text-fg-subtle')}>{t(`codecs.${item}.hint`)}</span>
              </button>
            ))}
          </div>
        ))}
      </nav>

      <div className="flex min-h-0 flex-col gap-3">
        <section className="flex shrink-0 flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
          <SegmentedControl<Direction>
            value={direction}
            onValueChange={(value) => {
              setDirection(value)
              setShowFile(false)
            }}
            aria-label={t('direction.label')}
            options={[
              { value: 'encode', label: t('direction.encode') },
              { value: 'decode', label: t('direction.decode') },
            ]}
          />
          <CodecOptions codec={codec} direction={direction} options={options} onChange={(patch) => setOptions((prev) => ({ ...prev, ...patch }))} />
          <div className="ml-auto flex items-center gap-2">
            {BASE64_CODECS.includes(codec) && direction === 'encode' && (
              <>
                <SegmentedControl<FileFormat>
                  size="sm"
                  value={fileFormat}
                  onValueChange={setFileFormat}
                  aria-label={t('file.format')}
                  options={[
                    { value: 'dataUri', label: 'Data URI' },
                    { value: 'base64', label: 'Base64' },
                  ]}
                />
                <Button onClick={encodeFile} loading={task.running}>
                  <FileUp />
                  {t('file.encode')}
                </Button>
              </>
            )}
            <Tooltip content={t('swap')}>
              <Button size="icon-md" aria-label={t('swap')} onClick={swap} disabled={!result || !!error || !result.text || fileMode}>
                <ArrowUpDown />
              </Button>
            </Tooltip>
          </div>
        </section>

        <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
          <Panel
            icon={<FileText />}
            title={t(direction === 'encode' ? 'input.plain' : 'input.encoded')}
            actions={
              <Button size="sm" onClick={() => setInput('')}>
                {t('common:clear')}
              </Button>
            }
            footer={result && !error && !fileMode && <span>{t('bytes', { count: result.inputBytes })}</span>}
          >
            <CodeEditor value={input} onChange={(value) => { setInput(value); setShowFile(false) }} language="text" lineWrapping aria-label={t('input.plain')} />
          </Panel>

          <Panel
            icon={<Terminal />}
            title={fileMode ? t('file.title', { name: task.result?.name ?? '' }) : t(direction === 'encode' ? 'output.encoded' : 'output.decoded')}
            extra={(pending || task.running) && <Spinner className="size-3.5 text-fg-subtle" />}
            actions={
              <>
                {fileMode && (
                  <Tooltip content={t('file.close')}>
                    <Button size="icon-sm" aria-label={t('file.close')} onClick={() => setShowFile(false)}>
                      <X />
                    </Button>
                  </Tooltip>
                )}
                {file ? (
                  <>
                    <Button size="sm" variant="outline" onClick={saveFile}>
                      <Download />
                      {t('common:download')}
                    </Button>
                    <Button size="sm" variant="outline" onClick={copyFile}>
                      {copied ? <Check className="text-success" /> : <Copy />}
                      {t('common:copy')}
                    </Button>
                  </>
                ) : (
                  <CopyButton text={output} label={t('common:copy')} variant="outline" disabled={!output} />
                )}
              </>
            }
            footer={
              fileMode
                ? task.result && <span>{t('file.summary', { bytes: task.result.bytes, chars: task.result.chars, mime: task.result.mime })}</span>
                : result &&
                  !error && (
                    <>
                      <span>{t('bytes', { count: result.outputBytes })}</span>
                      <span className="ml-auto">{t('elapsed', { ms: result.elapsedMs })}</span>
                    </>
                  )
            }
            bodyClassName="flex flex-col"
          >
            {fileMode && task.running ? (
              <div className="m-auto w-64 space-y-2 text-center">
                <Progress value={percent} />
                <p className="text-xs text-fg-muted">{t('file.reading')}</p>
              </div>
            ) : fileMode && task.error ? (
              <p className="p-4 text-[13px] text-danger">{errorMessage(task.error)}</p>
            ) : error ? (
              <p className="p-4 text-[13px] text-danger">
                {errorMessage(error)}
                {typeof error.params?.detail === 'string' && <span className="mt-1 block font-mono text-xs text-fg-muted">{error.params.detail}</span>}
              </p>
            ) : (
              <>
                {result && !result.text && !fileMode && (
                  <p className="flex shrink-0 items-center gap-2 border-b border-border bg-warning-soft/60 px-3 py-2 text-xs text-warning">
                    <Info className="size-3.5" />
                    {t('binaryNotice')}
                  </p>
                )}
                {file?.truncated && (
                  <p className="flex shrink-0 items-center gap-2 border-b border-border bg-warning-soft/60 px-3 py-2 text-xs text-warning">
                    <Info className="size-3.5" />
                    {t('file.truncated', { shown: file.preview.length, total: file.chars })}
                  </p>
                )}
                <div className="min-h-0 flex-1">
                  <CodeEditor value={output} readOnly language="text" lineWrapping aria-label={t('output.encoded')} />
                </div>
              </>
            )}
          </Panel>
        </div>
      </div>
    </div>
  )
}
