import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Panel, Spinner, Switch, cn, toast } from '@toolforge/ui'
import { host, usePlugin, usePluginState, useTask, type AppError } from '@toolforge/plugin-ui-sdk'
import { AlertTriangle, ArrowRight, CheckCircle2, Download, FilePlus2, FileSpreadsheet, FileText, FileType2, FolderOpen, FolderSearch, Upload, X } from 'lucide-react'

type Mode = 'excelToWord' | 'wordToExcel' | 'wordToPdf' | 'pdfToWord'
interface Engine {
  path: string
  version: string | null
}
interface Item {
  input: string
  output: string | null
  error: AppError | null
}
interface Result {
  items: Item[]
  converted: number
  failed: number
}

const MODES: { mode: Mode; from: typeof FileText; to: typeof FileText; extensions: string[]; engine: 'no' | 'legacy' | 'yes' }[] = [
  { mode: 'excelToWord', from: FileSpreadsheet, to: FileText, extensions: ['xlsx', 'xlsm', 'xlsb', 'xls', 'ods'], engine: 'no' },
  { mode: 'wordToExcel', from: FileText, to: FileSpreadsheet, extensions: ['docx', 'doc', 'rtf', 'odt', 'wps'], engine: 'legacy' },
  { mode: 'wordToPdf', from: FileText, to: FileType2, extensions: ['docx', 'doc', 'rtf', 'odt', 'wps'], engine: 'yes' },
  { mode: 'pdfToWord', from: FileType2, to: FileText, extensions: ['pdf'], engine: 'yes' },
]
const DOWNLOAD = 'https://www.libreoffice.org/download/download-libreoffice/'

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path
const extension = (path: string) => (path.includes('.') ? path.split('.').pop()!.toLowerCase() : '')

function DocConverter() {
  const { t, call, errorMessage } = usePlugin()
  const [mode, setMode] = usePluginState<Mode>('mode', 'excelToWord')
  const [headerRow, setHeaderRow] = usePluginState('headerRow', true)
  const [sheetTitles, setSheetTitles] = usePluginState('sheetTitles', true)
  const [outputDir, setOutputDir] = useState<string | null>(null)
  const [files, setFiles] = useState<string[]>([])
  const [engine, setEngine] = useState<Engine | null | undefined>(undefined)
  const [hovering, setHovering] = useState(false)
  const task = useTask<Result>()
  const spec = MODES.find((m) => m.mode === mode)!

  useEffect(() => {
    call<Engine | null>('engine', {}).then(setEngine, () => setEngine(null))
  }, [call])

  const add = (paths: string[]) => {
    const accepted = paths.filter((p) => spec.extensions.includes(extension(p)))
    if (accepted.length < paths.length) toast.info(t('files.skipped', { count: paths.length - accepted.length }))
    setFiles((prev) => [...prev, ...accepted.filter((p) => !prev.includes(p))])
  }
  const addRef = useRef(add)
  useEffect(() => {
    addRef.current = add
  })
  useEffect(() => {
    const off = host.onFileDrop({ drop: (paths) => addRef.current(paths), over: setHovering })
    return () => {
      off.then((unlisten) => unlisten())
    }
  }, [])
  useEffect(() => {
    if (task.status === 'succeeded' && task.result) {
      const { converted, failed } = task.result
      if (failed > 0) toast.error(t('result.summary', { converted, failed }))
      else toast.success(t('result.summary', { converted, failed }))
    }
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const switchMode = (next: Mode) => {
    setMode(next)
    setFiles([])
  }
  const pick = async () => {
    try {
      add(await host.dialog.openFiles([{ name: t(`modes.${mode}.from`), extensions: spec.extensions }]))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickOutput = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setOutputDir(dir)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const start = () =>
    task.start('convert', {
      mode,
      inputs: files,
      outputDir,
      headerRow,
      sheetTitles,
      tableName: t('names.table'),
      textName: t('names.text'),
    })
  const needsEngine = spec.engine === 'yes' || (spec.engine === 'legacy' && files.some((f) => extension(f) !== 'docx'))
  const blocked = needsEngine && engine === null
  const results = new Map((task.result?.items ?? []).map((i) => [i.input, i]))

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_300px] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <div className="grid shrink-0 grid-cols-4 gap-2">
          {MODES.map(({ mode: m, from: From, to: To }) => (
            <button
              key={m}
              type="button"
              onClick={() => switchMode(m)}
              disabled={task.running}
              className={cn(
                'flex flex-col items-center gap-1.5 rounded-card border px-3 py-3 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                m === mode ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border bg-surface text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <span className="flex items-center gap-1.5">
                <From className="size-5" />
                <ArrowRight className="size-3.5" />
                <To className="size-5" />
              </span>
              <span className="text-[13px] font-medium">{t(`modes.${m}.name`)}</span>
            </button>
          ))}
        </div>

        <Panel
          icon={<FileText />}
          title={t('files.title')}
          extra={files.length > 0 && <Badge>{files.length}</Badge>}
          actions={
            <>
              <Button size="sm" onClick={pick} disabled={task.running}>
                <FilePlus2 />
                {t('files.add')}
              </Button>
              <Button size="icon-sm" variant="ghost" aria-label={t('files.clear')} disabled={task.running || files.length === 0} onClick={() => setFiles([])}>
                <X />
              </Button>
            </>
          }
          className="min-h-0 flex-1"
          bodyClassName="overflow-auto p-2"
        >
          {files.length === 0 ? (
            <button
              type="button"
              onClick={pick}
              className={cn(
                'flex h-full min-h-40 w-full flex-col items-center justify-center gap-2 rounded-card border-2 border-dashed text-center outline-none transition-colors',
                'focus-visible:ring-2 focus-visible:ring-ring',
                hovering ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border text-fg-muted hover:border-border-strong hover:bg-hover',
              )}
            >
              <Upload className="size-6" />
              <span className="text-[13px] font-medium">{t('files.drop', { kind: t(`modes.${mode}.from`) })}</span>
              <span className="text-xs text-fg-subtle">{spec.extensions.map((e) => `.${e}`).join(' ')}</span>
            </button>
          ) : (
            <ul className="space-y-0.5">
              {files.map((file) => {
                const result = results.get(file)
                return (
                  <li key={file} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                    <spec.from className="size-4 shrink-0 text-fg-muted" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[13px] text-fg" title={file}>
                        {baseName(file)}
                      </p>
                      {result?.error && <p className="truncate text-[11px] text-danger">{errorMessage(result.error)}</p>}
                      {result?.output && <p className="truncate text-[11px] text-fg-subtle" title={result.output}>{t('result.saved', { name: baseName(result.output) })}</p>}
                    </div>
                    {result?.output ? (
                      <>
                        <CheckCircle2 className="size-4 text-success" />
                        <Button size="icon-sm" variant="ghost" aria-label={t('result.show')} onClick={() => host.revealPath(result.output!).catch(() => {})}>
                          <FolderSearch />
                        </Button>
                      </>
                    ) : result?.error ? (
                      <AlertTriangle className="size-4 text-danger" />
                    ) : (
                      <Button size="icon-sm" variant="ghost" className="opacity-0 group-hover:opacity-100" aria-label={t('files.remove')} disabled={task.running} onClick={() => setFiles(files.filter((f) => f !== file))}>
                        <X />
                      </Button>
                    )}
                  </li>
                )
              })}
            </ul>
          )}
        </Panel>
      </div>

      <div className="flex min-h-0 flex-col gap-3">
        <Panel title={t('engine.title')} className="shrink-0" bodyClassName="space-y-2 p-3">
          {engine === undefined ? (
            <Spinner />
          ) : engine ? (
            <p className="flex items-center gap-1.5 text-[13px] text-fg">
              <CheckCircle2 className="size-4 text-success" />
              {t('engine.found', { version: engine.version ?? '' })}
            </p>
          ) : (
            <>
              <p className={cn('text-[13px]', spec.engine === 'no' ? 'text-fg-muted' : 'text-warning')}>{t('engine.missing')}</p>
              <Button size="sm" onClick={() => host.openUrl(DOWNLOAD).catch(() => {})}>
                <Download />
                {t('engine.download')}
              </Button>
            </>
          )}
          <p className="text-[11px] leading-snug text-fg-subtle">{t(`modes.${mode}.engine`)}</p>
        </Panel>

        <Panel title={t('options.title')} className="min-h-0 flex-1" bodyClassName="space-y-3 overflow-auto p-3">
          {(mode === 'excelToWord' || mode === 'wordToExcel') && (
            <label className="flex items-center justify-between gap-2 text-[13px] text-fg">
              {t('options.headerRow')}
              <Switch size="sm" checked={headerRow} onCheckedChange={setHeaderRow} aria-label={t('options.headerRow')} />
            </label>
          )}
          {mode === 'excelToWord' && (
            <label className="flex items-center justify-between gap-2 text-[13px] text-fg">
              {t('options.sheetTitles')}
              <Switch size="sm" checked={sheetTitles} onCheckedChange={setSheetTitles} aria-label={t('options.sheetTitles')} />
            </label>
          )}
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('options.output')}</p>
            <Button block size="sm" className="justify-start" onClick={pickOutput}>
              <FolderOpen />
              <span className="truncate">{outputDir ?? t('options.sameFolder')}</span>
            </Button>
            {outputDir && (
              <Button size="sm" variant="ghost" onClick={() => setOutputDir(null)}>
                {t('options.useSameFolder')}
              </Button>
            )}
          </div>
          <p className="text-[11px] leading-snug text-fg-subtle">{t(`modes.${mode}.hint`)}</p>
        </Panel>

        <Button variant="primary" loading={task.running} disabled={files.length === 0 || blocked} onClick={start}>
          {t('convert', { count: files.length })}
        </Button>
        {blocked && <p className="text-[11px] text-warning">{t('engine.required')}</p>}
      </div>
    </div>
  )
}

export default DocConverter
