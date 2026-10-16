import { useState } from 'react'
import { Badge, Button, CodeEditor, DropdownMenu, Empty, Input, Panel, SegmentedControl, Tooltip, cn, toast } from '@toolforge/ui'
import { host, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Check, ChevronDown, Download, FilePlus2, FolderOpen, ListTree, Play, ScanSearch, TriangleAlert, X } from 'lucide-react'
import { fileLabel, type ConfigInput, type Output, type Property, type TargetResult } from './types'

type Mode = 'project' | 'paste'

const SAMPLE_CONFIG = `root = true

[*]
indent_style = space
indent_size = 4
end_of_line = lf
insert_final_newline = true

[*.{js,ts,tsx}]
indent_size = 2

[Makefile]
indent_style = tab`

function PropertyRow({ prop }: { prop: Property }) {
  const { t } = usePlugin()
  const overridden = prop.trace.slice(0, -1)
  return (
    <tr className="border-t border-border align-top">
      <td className="px-2 py-1.5">
        <code className="text-fg">{prop.key}</code>
        {prop.kind !== 'standard' && (
          <Badge className="ml-1.5" variant={prop.kind === 'derived' ? 'info' : 'neutral'}>
            {t(`kind.${prop.kind}`)}
          </Badge>
        )}
      </td>
      <td className="px-2 py-1.5">
        {prop.value === null ? (
          <span className="text-fg-subtle">{t('result.unsetValue')}</span>
        ) : (
          <span className="flex items-center gap-1.5">
            <code className={cn('font-semibold', prop.invalid ? 'text-danger' : 'text-fg')}>{prop.value}</code>
            {prop.invalid && (
              <Tooltip content={t('result.invalidHint')}>
                <TriangleAlert className="size-3.5 text-danger" />
              </Tooltip>
            )}
          </span>
        )}
      </td>
      <td className="px-2 py-1.5 text-fg-muted">
        {prop.source ? (
          <span className="font-mono">{t('result.sourceText', { file: fileLabel(prop.source.dir), line: prop.source.line, section: prop.source.section })}</span>
        ) : prop.kind === 'derived' ? (
          t('result.derived')
        ) : (
          t('result.removedByUnset')
        )}
        {overridden.length > 0 && (
          <ul className="mt-1 space-y-0.5 text-[11px] text-fg-subtle">
            {overridden.map((step, index) => (
              <li key={index} className="font-mono line-through decoration-fg-subtle/60">
                {t('result.overriddenBy', {
                  value: step.unset ? t('result.unsetValue') : step.value,
                  file: fileLabel(step.source.dir),
                  line: step.source.line,
                  section: step.source.section,
                })}
              </li>
            ))}
          </ul>
        )}
      </td>
    </tr>
  )
}

function TargetCard({ result }: { result: TargetResult }) {
  const { t } = usePlugin()
  const [showSections, setShowSections] = useState(false)
  return (
    <section className="space-y-2 rounded-card border border-border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <code className="text-[13px] font-semibold text-fg" data-selectable>
          {result.path}
        </code>
        <span className="text-[11px] text-fg-subtle">
          {t('result.chain', { list: result.files.length > 0 ? result.files.map(fileLabel).join(' → ') : t('result.noFiles') })}
        </span>
      </div>
      {result.incomplete && (
        <p className="flex items-start gap-1.5 rounded-control bg-warning-soft px-2.5 py-1.5 text-xs text-warning">
          <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
          {t('result.incomplete')}
        </p>
      )}
      {result.properties.length === 0 ? (
        <p className="text-xs text-fg-subtle">{t('result.noProperties')}</p>
      ) : (
        <table className="w-full border-collapse text-xs">
          <thead className="text-fg-muted">
            <tr>
              <th className="w-56 px-2 py-1 text-left font-medium">{t('result.property')}</th>
              <th className="w-32 px-2 py-1 text-left font-medium">{t('result.value')}</th>
              <th className="px-2 py-1 text-left font-medium">{t('result.source')}</th>
            </tr>
          </thead>
          <tbody>
            {result.properties.map((prop) => (
              <PropertyRow key={prop.key} prop={prop} />
            ))}
          </tbody>
        </table>
      )}
      {result.unsetStandard.length > 0 && (
        <p className="text-xs text-fg-muted">
          {t('result.notSet')}{' '}
          {result.unsetStandard.map((key) => (
            <code key={key} className="mr-1.5 text-fg-subtle">
              {key}
            </code>
          ))}
        </p>
      )}
      <Button size="sm" variant="ghost" onClick={() => setShowSections(!showSections)}>
        <ListTree />
        {t(showSections ? 'result.hideSections' : 'result.showSections', { count: result.sections.length })}
      </Button>
      {showSections && (
        <ul className="space-y-0.5 text-xs">
          {result.sections.map((section) => (
            <li key={`${section.dir}-${section.line}`} className={cn('flex items-center gap-2 font-mono', section.matched ? 'text-fg' : 'text-fg-subtle')}>
              {section.matched ? <Check className="size-3.5 text-success" /> : <X className="size-3.5" />}
              {t('result.sourceText', { file: fileLabel(section.dir), line: section.line, section: section.glob })}
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function EditorConfigTool() {
  const { t, call, errorMessage } = usePlugin()
  const [mode, setMode] = usePluginState<Mode>('mode', 'project')
  const [root, setRoot] = usePluginState('root', '')
  const [targetsText, setTargetsText] = usePluginState('targets', 'src/main.ts\nMakefile')
  const [pasted, setPasted] = usePluginState('pasted', SAMPLE_CONFIG)
  const [output, setOutput] = useState<Output | null>(null)
  const [configs, setConfigs] = useState<ConfigInput[]>([])
  const [busy, setBusy] = useState(false)

  const targets = targetsText
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)

  const pickRoot = async () => {
    try {
      const dir = await host.dialog.openDirectory()
      if (dir) setRoot(dir)
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const addFiles = async () => {
    try {
      const paths = await host.dialog.openFiles()
      if (paths.length === 0) return
      const relative = await call<string[]>('relative', { root, paths })
      setTargetsText([...targets, ...relative.filter((p) => !targets.includes(p))].join('\n'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const inspect = async () => {
    setBusy(true)
    try {
      let read: ConfigInput[]
      if (mode === 'paste') {
        read = [{ dir: '', text: pasted }]
      } else {
        const files = await call<{ dir: string; path: string }[]>('chain', { root, targets })
        read = []
        for (const file of files) {
          // 不存在的 .editorconfig 直接跳过；只读取项目目录内的这些路径
          try {
            read.push({ dir: file.dir, text: await host.fs.readText(file.path) })
          } catch {
            // 该目录没有配置
          }
        }
      }
      setConfigs(read)
      setOutput(await call<Output>('resolve', { targets, configs: read }))
    } catch (reason) {
      setOutput(null)
      toast.error(errorMessage(reason))
    } finally {
      setBusy(false)
    }
  }

  const exportReport = async (format: 'json' | 'markdown', target: 'copy' | 'save') => {
    try {
      const text = await call<string>('report', { targets, configs, format })
      if (target === 'copy') {
        await host.clipboard.writeText(text)
        toast.success(t('common:copied'))
        return
      }
      const ext = format === 'json' ? 'json' : 'md'
      const path = await host.dialog.saveFile(`editorconfig-report.${ext}`, [{ name: format.toUpperCase(), extensions: [ext] }])
      if (!path) return
      await host.fs.writeText(path, text)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const canRun = targets.length > 0 && (mode === 'paste' ? pasted.trim().length > 0 : root.trim().length > 0)

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,0.8fr)_minmax(0,1.3fr)] gap-3">
      <Panel
        icon={<ScanSearch />}
        title={t('input.title')}
        actions={
          <SegmentedControl<Mode>
            size="sm"
            value={mode}
            onValueChange={setMode}
            aria-label={t('input.mode')}
            options={[
              { value: 'project', label: t('input.project') },
              { value: 'paste', label: t('input.paste') },
            ]}
          />
        }
        footer={
          <Button variant="primary" size="sm" className="ml-auto" disabled={!canRun || busy} loading={busy} onClick={() => void inspect()}>
            <Play />
            {t('input.inspect')}
          </Button>
        }
        bodyClassName="flex min-h-0 flex-col gap-3 overflow-auto p-3"
      >
        {mode === 'project' ? (
          <div className="space-y-1">
            <p className="text-xs text-fg-muted">{t('input.root')}</p>
            <div className="flex gap-2">
              <Input size="sm" className="flex-1 font-mono" value={root} onChange={(event) => setRoot(event.target.value)} placeholder={t('input.rootPlaceholder')} aria-label={t('input.root')} />
              <Button size="sm" onClick={() => void pickRoot()}>
                <FolderOpen />
                {t('input.choose')}
              </Button>
            </div>
            <p className="text-[11px] text-fg-subtle">{t('input.rootHint')}</p>
          </div>
        ) : (
          <div className="flex min-h-48 flex-1 flex-col gap-1">
            <p className="text-xs text-fg-muted">{t('input.pasted')}</p>
            <div className="min-h-0 flex-1">
              <CodeEditor value={pasted} onChange={setPasted} language="text" aria-label={t('input.pasted')} />
            </div>
          </div>
        )}
        <div className="flex min-h-32 flex-col gap-1">
          <div className="flex items-center justify-between">
            <p className="text-xs text-fg-muted">{t('input.targets')}</p>
            {mode === 'project' && (
              <Button size="sm" variant="ghost" disabled={!root.trim()} onClick={() => void addFiles()}>
                <FilePlus2 />
                {t('input.addFiles')}
              </Button>
            )}
          </div>
          <div className="min-h-24 flex-1">
            <CodeEditor value={targetsText} onChange={setTargetsText} language="text" placeholder={t('input.targetsPlaceholder')} aria-label={t('input.targets')} />
          </div>
          <p className="text-[11px] text-fg-subtle">{t('input.targetsHint')}</p>
        </div>
      </Panel>

      <Panel
        icon={<ListTree />}
        title={t('result.title')}
        extra={output && <Badge>{t('result.spec', { version: output.specVersion })}</Badge>}
        actions={
          <DropdownMenu
            trigger={
              <Button size="sm" disabled={!output}>
                <Download />
                {t('export.title')}
                <ChevronDown />
              </Button>
            }
            items={[
              { key: 'copy-md', label: t('export.copyMarkdown'), onSelect: () => void exportReport('markdown', 'copy') },
              { key: 'save-md', label: t('export.saveMarkdown'), onSelect: () => void exportReport('markdown', 'save') },
              { key: 'copy-json', label: t('export.copyJson'), onSelect: () => void exportReport('json', 'copy') },
              { key: 'save-json', label: t('export.saveJson'), onSelect: () => void exportReport('json', 'save') },
            ]}
          />
        }
        bodyClassName="space-y-3 overflow-auto p-3"
      >
        {!output ? (
          <Empty icon={<ListTree />} title={t('result.empty')} description={t('result.emptyHint')} />
        ) : (
          <>
            {output.problems.length > 0 && (
              <div className="space-y-1 rounded-control bg-danger-soft px-3 py-2 text-xs text-danger">
                <p className="font-medium">{t('problems.title', { count: output.problems.length })}</p>
                <ul className="space-y-0.5 font-mono">
                  {output.problems.map((p, index) => (
                    <li key={index}>
                      {t('problems.item', { file: fileLabel(p.dir), line: p.line, problem: t(`problems.${p.kind}`) })} <span className="opacity-80">{p.text}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {output.targets.map((result) => (
              <TargetCard key={result.path} result={result} />
            ))}
          </>
        )}
      </Panel>
    </div>
  )
}

export default EditorConfigTool
