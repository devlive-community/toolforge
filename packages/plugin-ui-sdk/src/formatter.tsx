import { useRef, useState, type ReactNode } from 'react'
import {
  Button,
  CodeEditor,
  Panel,
  Spinner,
  Switch,
  Tabs,
  Tooltip,
  toast,
  type CodeEditorHandle,
  type CodeEditorProps,
} from '@toolforge/ui'
import { ClipboardPaste, Crosshair, Download, FileText, FolderOpen, Terminal, TriangleAlert, Wand2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { CopyButton } from './copy'
import { host, type FileFilter } from './host'
import { useErrorMessage } from './i18n'
import { usePlugin } from './context'
import type { AppError } from './types'
import { useDebouncedCall } from './useDebouncedCall'

export interface FormatterMode {
  /** 插件后端函数名 */
  fn: string
  label: string
  icon?: ReactNode
  outputTitle: string
}

export interface FormatterResult {
  output: string
  stats?: { lines: number; chars: number }
  elapsedMs: number
}

export interface FormatterToolProps<R extends FormatterResult> {
  language: CodeEditorProps['language']
  modes: FormatterMode[]
  inputTitle: string
  sample: string
  placeholder?: string
  fileFilters: FileFilter[]
  downloadName: string
  /** 额外参数（缩进、大小写等）；变化时自动重新处理 */
  args?: Record<string, unknown>
  /** 输出面板底栏右侧的插件选项 */
  options?: ReactNode
  /** 输出面板底栏左侧的额外统计 */
  summary?: (result: R) => ReactNode
}

function ErrorNotice({ error, onLocate }: { error: AppError; onLocate?: (line: number, column: number) => void }) {
  const { t } = useTranslation('common')
  const { manifest } = usePlugin()
  const errorMessage = useErrorMessage(manifest.id)
  const line = Number(error.params?.line ?? 0)
  const column = Number(error.params?.column ?? 0)
  const detail = typeof error.params?.detail === 'string' ? error.params.detail : null
  return (
    <div className="flex h-full items-start justify-center overflow-auto p-6">
      <div className="w-full max-w-md animate-fade-in rounded-card border border-danger/30 bg-danger-soft/60 p-4">
        <div className="flex items-center gap-2 text-danger">
          <TriangleAlert className="size-4" />
          <p className="text-[13px] font-semibold">{t('formatter.errorTitle')}</p>
        </div>
        <p className="mt-2 text-[13px] leading-relaxed text-fg" data-selectable>
          {errorMessage(error)}
        </p>
        {detail && (
          <p className="mt-2 rounded-control bg-surface/70 px-2.5 py-1.5 font-mono text-xs break-all text-fg-muted" data-selectable>
            {detail}
          </p>
        )}
        {line > 0 && onLocate && (
          <Button size="sm" className="mt-3" onClick={() => onLocate(line, column)}>
            <Crosshair />
            {t('formatter.locate')}
          </Button>
        )}
      </div>
    </div>
  )
}

/**
 * 通用「输入 → 输出」格式化工具布局：模式切换、输入 / 输出编辑器、粘贴 / 打开 / 复制 / 下载、
 * 错误定位。所有处理都通过插件后端函数完成。
 */
export function FormatterTool<R extends FormatterResult>({
  language,
  modes,
  inputTitle,
  sample,
  placeholder,
  fileFilters,
  downloadName,
  args = {},
  options,
  summary,
}: FormatterToolProps<R>) {
  const { t } = useTranslation('common')
  const { manifest } = usePlugin()
  const errorMessage = useErrorMessage(manifest.id)
  const [mode, setMode] = useState(modes[0].fn)
  const [input, setInput] = useState(sample)
  const [wrap, setWrap] = useState(false)
  const inputRef = useRef<CodeEditorHandle>(null)

  const hasInput = input.trim().length > 0
  const extra = Object.values(args)
  const { result, error, pending } = useDebouncedCall<R>(hasInput ? mode : null, hasInput ? { input, ...args } : null, [mode, input, ...extra])
  const current = modes.find((m) => m.fn === mode) ?? modes[0]
  const errorLine = error ? Number(error.params?.line ?? 0) || null : null

  const run = async (action: () => Promise<unknown>) => {
    try {
      await action()
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      {modes.length > 1 && (
        <Tabs
          value={mode}
          onValueChange={setMode}
          items={modes.map((m) => ({ value: m.fn, label: m.label, icon: m.icon }))}
          className="self-start"
        />
      )}
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel
          icon={<FileText />}
          title={inputTitle}
          extra={
            <Button variant="soft" size="sm" className="ml-1" onClick={() => setInput(sample)}>
              <Wand2 />
              {t('formatter.sample')}
            </Button>
          }
          actions={
            <>
              <Button size="sm" onClick={() => setInput('')}>
                {t('clear')}
              </Button>
              <Button size="sm" onClick={() => run(async () => setInput(await host.clipboard.readText()))}>
                <ClipboardPaste />
                {t('paste')}
              </Button>
              <Tooltip content={t('formatter.open')}>
                <Button
                  size="icon-sm"
                  aria-label={t('formatter.open')}
                  onClick={() =>
                    run(async () => {
                      const path = await host.dialog.openFile(fileFilters)
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
            <>
              <span>{t('formatter.chars', { count: input.length })}</span>
              <label className="ml-auto flex items-center gap-2">
                {t('formatter.wrap')}
                <Switch size="sm" checked={wrap} onCheckedChange={setWrap} aria-label={t('formatter.wrap')} />
              </label>
            </>
          }
        >
          <CodeEditor
            ref={inputRef}
            value={input}
            onChange={setInput}
            language={language}
            lineWrapping={wrap}
            errorLine={errorLine}
            placeholder={placeholder}
            aria-label={inputTitle}
          />
        </Panel>

        <Panel
          icon={<Terminal />}
          title={current.outputTitle}
          extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            <>
              <CopyButton text={result?.output ?? ''} label={t('copy')} variant="outline" disabled={!result || !!error} />
              <Button
                size="sm"
                disabled={!result || !!error}
                onClick={() =>
                  run(async () => {
                    if (!result) return
                    const path = await host.dialog.saveFile(downloadName, fileFilters)
                    if (!path) return
                    await host.fs.writeText(path, result.output)
                    toast.success(t('saved'))
                  })
                }
              >
                <Download />
                {t('download')}
              </Button>
            </>
          }
          footer={
            <>
              {result && !error && (
                <>
                  {result.stats && <span>{t('formatter.lines', { count: result.stats.lines })}</span>}
                  {result.stats && <span>{t('formatter.chars', { count: result.stats.chars })}</span>}
                  {summary?.(result)}
                  <span>{t('formatter.elapsed', { ms: result.elapsedMs })}</span>
                </>
              )}
              <div className="ml-auto flex items-center gap-3">{options}</div>
            </>
          }
        >
          {!hasInput ? (
            <p className="p-6 text-center text-[13px] text-fg-subtle">{t('formatter.waiting')}</p>
          ) : error ? (
            <ErrorNotice error={error} onLocate={(line, column) => inputRef.current?.gotoLine(line, column)} />
          ) : !result ? (
            <div className="flex h-full items-center justify-center text-fg-muted">
              <Spinner />
            </div>
          ) : (
            <CodeEditor value={result.output} readOnly language={language} lineWrapping={wrap || mode === 'minify'} aria-label={current.outputTitle} />
          )}
        </Panel>
      </div>
    </div>
  )
}
