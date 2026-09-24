import { Badge, Button, CodeEditor, Panel, SegmentedControl, Spinner, toast, type CodeLanguage } from '@toolforge/ui'
import { CopyButton, ValueRow, host, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ClipboardPaste, Code2, Download, Send, Terminal, TriangleAlert } from 'lucide-react'
import type { Body, Converted, Language } from './types'

const LANGUAGES: { value: Language; label: string; ext: string; editor: CodeLanguage }[] = [
  { value: 'javascript', label: 'JavaScript', ext: 'mjs', editor: 'typescript' },
  { value: 'python', label: 'Python', ext: 'py', editor: 'python' },
  { value: 'go', label: 'Go', ext: 'go', editor: 'go' },
  { value: 'rust', label: 'Rust', ext: 'rs', editor: 'rust' },
  { value: 'java', label: 'Java', ext: 'java', editor: 'java' },
  { value: 'php', label: 'PHP', ext: 'php', editor: 'text' },
  { value: 'csharp', label: 'C#', ext: 'cs', editor: 'csharp' },
]

const SAMPLE = `curl 'https://api.example.com/v1/users?page=2' \\
  -X POST \\
  -H 'Authorization: Bearer <token>' \\
  -H 'Content-Type: application/json' \\
  -d '{"name":"Ada","roles":["admin"],"active":true}' \\
  --compressed -L`

export function CurlConverter() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = usePluginState('command', SAMPLE)
  const [language, setLanguage] = usePluginState<Language>('language', 'python')

  const args = input.trim() ? { command: input, language } : null
  const { result, error, pending } = useDebouncedCall<Converted>(args ? 'convert' : null, args, [input, language])
  const output = error ? null : result
  const meta = LANGUAGES.find((l) => l.value === language) ?? LANGUAGES[0]

  const run = async (action: () => Promise<void>) => {
    try {
      await action()
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  const paste = () =>
    run(async () => {
      const text = await host.clipboard.readText()
      if (text) setInput(text)
    })

  const save = () =>
    run(async () => {
      if (!output) return
      const name = language === 'java' ? 'Main' : 'request'
      const path = await host.dialog.saveFile(`${name}.${meta.ext}`)
      if (!path) return
      await host.fs.writeText(path, output.code)
      toast.success(t('common:saved'))
    })

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <SegmentedControl<Language>
          value={language}
          onValueChange={setLanguage}
          aria-label={t('language')}
          options={LANGUAGES.map(({ value, label }) => ({ value, label }))}
        />
      </section>

      <div className="grid min-h-0 grid-cols-2 gap-3">
        <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
          <Panel
            icon={<Terminal />}
            title={t('input.title')}
            actions={
              <>
                <Button size="sm" onClick={paste}>
                  <ClipboardPaste />
                  {t('input.paste')}
                </Button>
                <Button size="sm" onClick={() => setInput('')}>
                  {t('common:clear')}
                </Button>
              </>
            }
            footer={error && <span className="truncate text-danger">{errorMessage(error)}</span>}
          >
            <CodeEditor value={input} onChange={setInput} language="text" lineWrapping placeholder={t('input.placeholder')} aria-label={t('input.title')} />
          </Panel>
          {output && <RequestPanel converted={output} />}
        </div>

        <Panel
          icon={<Code2 />}
          title={t('output.title')}
          extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            <>
              <Button size="sm" variant="outline" disabled={!output} onClick={save}>
                <Download />
                {t('output.save')}
              </Button>
              <CopyButton text={output?.code ?? ''} label={t('common:copy')} variant="outline" disabled={!output} />
            </>
          }
        >
          <CodeEditor value={output?.code ?? ''} readOnly language={meta.editor} aria-label={t('output.title')} />
        </Panel>
      </div>
    </div>
  )
}

function bodyLabel(body: Body, t: ReturnType<typeof usePlugin>['t']) {
  switch (body.kind) {
    case 'none':
      return t('request.kinds.none')
    case 'text':
      return t('request.kinds.text', { count: body.text.length })
    case 'file':
      return t('request.kinds.file', { path: body.path })
    case 'multipart':
      return t('request.kinds.multipart', { count: body.parts.length })
  }
}

function RequestPanel({ converted }: { converted: Converted }) {
  const { t, errorMessage } = usePlugin()
  const { request, warnings } = converted
  const flags = [
    request.insecure && t('request.flags.insecure'),
    request.followRedirects && t('request.flags.follow'),
    request.compressed && t('request.flags.compressed'),
    request.timeout != null && t('request.flags.timeout', { seconds: request.timeout }),
    request.proxy && t('request.flags.proxy', { proxy: request.proxy }),
  ].filter((flag): flag is string => typeof flag === 'string')

  return (
    <Panel icon={<Send />} title={t('request.title')} className="max-h-[45vh]" bodyClassName="overflow-auto py-1.5">
      <div className="flex items-start gap-2 px-3 py-1.5">
        <Badge variant="info" className="shrink-0 font-mono">
          {request.method}
        </Badge>
        <code className="min-w-0 flex-1 font-mono text-[12.5px] break-all text-fg" data-selectable>
          {request.url}
        </code>
      </div>
      {flags.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-3 py-1.5">
          {flags.map((flag) => (
            <Badge key={flag}>{flag}</Badge>
          ))}
        </div>
      )}
      {warnings.length > 0 && (
        <div className="mx-3 my-1.5 space-y-1 rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">
          <p className="flex items-center gap-1.5 font-semibold">
            <TriangleAlert className="size-3.5" />
            {t('warnings.title')}
          </p>
          {warnings.map((warning, index) => (
            <p key={index}>{t(`warnings.${warning.code}`, { ...warning.params, defaultValue: errorMessage(warning) })}</p>
          ))}
        </div>
      )}
      <p className="px-3 pt-2 pb-1 text-xs font-semibold text-fg-muted">{t('request.headers')}</p>
      {request.headers.length === 0 ? (
        <p className="px-3 py-1 text-xs text-fg-subtle">{t('request.noHeaders')}</p>
      ) : (
        request.headers.map(([name, value], index) => <ValueRow key={`${name}-${index}`} label={name} value={value} labelWidth="w-36" />)
      )}
      <p className="px-3 pt-2 pb-1 text-xs font-semibold text-fg-muted">{t('request.body')}</p>
      <p className="px-3 py-1 text-xs text-fg">{bodyLabel(request.body, t)}</p>
    </Panel>
  )
}
