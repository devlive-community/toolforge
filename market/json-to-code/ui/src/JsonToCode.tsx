import { useState } from 'react'
import { Badge, Button, CodeEditor, Input, Panel, SegmentedControl, Spinner, toast, type CodeLanguage } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Braces, Code2, Download } from 'lucide-react'

type Language = 'typescript' | 'rust' | 'go' | 'java' | 'kotlin' | 'python' | 'csharp'

const LANGUAGES: { value: Language; label: string; ext: string }[] = [
  { value: 'typescript', label: 'TypeScript', ext: 'ts' },
  { value: 'rust', label: 'Rust', ext: 'rs' },
  { value: 'go', label: 'Go', ext: 'go' },
  { value: 'java', label: 'Java', ext: 'java' },
  { value: 'kotlin', label: 'Kotlin', ext: 'kt' },
  { value: 'python', label: 'Python', ext: 'py' },
  { value: 'csharp', label: 'C#', ext: 'cs' },
]

const SAMPLE = `{
  "id": 42,
  "userName": "ada",
  "email": "ada@example.com",
  "active": true,
  "score": 98.5,
  "tags": ["admin", "dev"],
  "profile": { "avatar_url": "https://example.com/a.png", "bio": null },
  "posts": [
    { "id": 1, "title": "Hello", "published_at": "2026-01-01T00:00:00Z" },
    { "id": 2, "title": "World", "draft": true }
  ]
}`

interface Output {
  code: string
  language: Language
  types: number
}

export function JsonToCode() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState(SAMPLE)
  const [language, setLanguage] = useState<Language>('typescript')
  const [root, setRoot] = useState('User')
  useLaunchInput((text) => setInput(text))

  const args = input.trim() ? { input, language, root } : null
  const { result, error, pending } = useDebouncedCall<Output>(args ? 'generate' : null, args, [input, language, root])
  const output = error ? null : result
  const ext = LANGUAGES.find((l) => l.value === language)?.ext ?? 'txt'

  const save = async () => {
    if (!output) return
    try {
      const path = await host.dialog.saveFile(`${root || 'Root'}.${ext}`)
      if (!path) return
      await host.fs.writeText(path, output.code)
      toast.success(t('common:saved'))
    } catch (reason) {
      toast.error(errorMessage(reason))
    }
  }

  return (
    <div className="grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
      <section className="flex flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <SegmentedControl<Language> value={language} onValueChange={setLanguage} aria-label={t('language')} options={LANGUAGES.map(({ value, label }) => ({ value, label }))} />
        <label className="ml-auto flex items-center gap-2 text-xs text-fg-muted">
          {t('root')}
          <Input size="sm" value={root} onChange={(event) => setRoot(event.target.value)} className="font-mono" wrapperClassName="w-40" aria-label={t('root')} spellCheck={false} />
        </label>
      </section>

      <div className="grid min-h-0 grid-cols-2 gap-3">
        <Panel
          icon={<Braces />}
          title={t('input.title')}
          actions={
            <Button size="sm" onClick={() => setInput('')}>
              {t('common:clear')}
            </Button>
          }
          footer={error && <span className="truncate text-danger">{errorMessage(error)}</span>}
        >
          <CodeEditor value={input} onChange={setInput} language="json" errorLine={typeof error?.params?.line === 'number' ? error.params.line : null} placeholder={t('input.placeholder')} aria-label={t('input.title')} />
        </Panel>
        <Panel
          icon={<Code2 />}
          title={t('output.title')}
          extra={pending ? <Spinner className="size-3.5 text-fg-subtle" /> : output && <Badge>{t('output.types', { count: output.types })}</Badge>}
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
          <CodeEditor value={output?.code ?? ''} readOnly language={language as CodeLanguage} aria-label={t('output.title')} />
        </Panel>
      </div>
    </div>
  )
}
