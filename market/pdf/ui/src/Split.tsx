import { useEffect, useRef, useState } from 'react'
import { Badge, Button, Empty, Input, NumberInput, Panel, SegmentedControl, toast } from '@toolforge/ui'
import { formatBytes, host, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileText, FolderOpen, FolderSearch, Scissors, X } from 'lucide-react'
import { usePdfOpener } from './usePasswordPrompt'
import { fileName, type Info } from './types'

type Mode = 'ranges' | 'every' | 'single'

export function Split({ dropped }: { dropped: string | null }) {
  const { t, errorMessage } = usePlugin()
  const { open, dialog } = usePdfOpener()
  const [file, setFile] = useState<{ info: Info; password: string | null } | null>(null)
  const [mode, setMode] = useState<Mode>('ranges')
  const [ranges, setRanges] = useState('')
  const [size, setSize] = useState(2)
  const [dir, setDir] = useState<string | null>(null)
  const task = useTask<{ outputs: string[]; dir: string }>()

  const load = async (path: string) => {
    const result = await open(path)
    if ('error' in result) {
      if (result.error) toast.error(result.error)
      return
    }
    setFile(result)
    setRanges(`1-${Math.ceil(result.info.pages.length / 2)}, ${Math.ceil(result.info.pages.length / 2) + 1}-`)
  }
  const loadRef = useRef(load)
  useEffect(() => {
    loadRef.current = load
  })
  useEffect(() => {
    if (dropped) void loadRef.current(dropped)
  }, [dropped])

  useEffect(() => {
    if (task.status === 'succeeded' && task.result) toast.success(t('split.done', { count: task.result.outputs.length }))
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status]) // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    try {
      const path = await host.dialog.openFile([{ name: 'PDF', extensions: ['pdf'] }])
      if (path) await load(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const pickDir = async () => {
    try {
      const path = await host.dialog.openDirectory()
      if (path) setDir(path)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const start = () =>
    file && task.start('split', { path: file.info.path, password: file.password, mode, size, ranges, outputDir: dir })

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <Panel icon={<Scissors />} title={t('split.title')} bodyClassName="space-y-4 overflow-auto p-3">
        {file ? (
          <div className="flex items-center gap-2 rounded-control border border-border px-3 py-2">
            <FileText className="size-4 text-fg-muted" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[13px] text-fg">{file.info.name}</p>
              <p className="text-[11px] text-fg-subtle">{t('organize.fileInfo', { pages: file.info.pages.length, size: formatBytes(file.info.size), version: file.info.version })}</p>
            </div>
            <Button size="icon-sm" variant="ghost" aria-label={t('organize.removeFile')} onClick={() => setFile(null)}>
              <X />
            </Button>
          </div>
        ) : (
          <Button block onClick={pick}>
            <FileText />
            {t('split.choose')}
          </Button>
        )}
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('split.mode')}</p>
          <SegmentedControl<Mode>
            size="sm"
            value={mode}
            onValueChange={setMode}
            aria-label={t('split.mode')}
            options={(['ranges', 'every', 'single'] as Mode[]).map((value) => ({ value, label: t(`split.modes.${value}`) }))}
          />
        </div>
        {mode === 'ranges' && (
          <div className="space-y-1.5">
            <Input value={ranges} onChange={(e) => setRanges(e.target.value)} placeholder="1-3, 5, 8-" className="font-mono" aria-label={t('split.ranges')} />
            <p className="text-[11px] text-fg-subtle">{t('split.rangesHint')}</p>
          </div>
        )}
        {mode === 'every' && (
          <label className="flex items-center gap-2 text-[13px] text-fg">
            {t('split.everyLabel')}
            <NumberInput value={size} onValueChange={setSize} min={1} max={1000} className="w-28" aria-label={t('split.everyLabel')} />
          </label>
        )}
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('split.output')}</p>
          <Button block className="justify-start" onClick={pickDir}>
            <FolderOpen />
            <span className="truncate">{dir ?? t('split.sameDir')}</span>
          </Button>
        </div>
        <Button block variant="primary" loading={task.running} disabled={!file} onClick={start}>
          <Scissors />
          {t('split.start')}
        </Button>
      </Panel>
      <Panel title={t('split.results')} extra={task.result && <Badge variant="success">{task.result.outputs.length}</Badge>} bodyClassName="overflow-auto p-2">
        {!task.result ? (
          <Empty icon={<Scissors />} title={t('split.empty')} />
        ) : (
          <ul className="space-y-0.5">
            {task.result.outputs.map((output) => (
              <li key={output} className="group flex items-center gap-2 rounded-control px-2.5 py-1.5 hover:bg-hover">
                <FileText className="size-4 text-fg-muted" />
                <span className="min-w-0 flex-1 truncate text-[13px] text-fg" title={output}>{fileName(output)}</span>
                <Button size="icon-sm" variant="ghost" aria-label={t('organize.show')} onClick={() => host.revealPath(output).catch(() => {})}>
                  <FolderSearch />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </Panel>
      {dialog}
    </div>
  )
}
