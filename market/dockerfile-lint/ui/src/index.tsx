import { useState } from 'react'
import { Badge, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Container } from 'lucide-react'

interface Issue {
  line: number
  rule: string
  severity: string
  fix: string
}
interface LintOut {
  issues: Issue[]
  stages: number
}

const SAMPLE = 'FROM ubuntu\nADD ./app /app\nUSER root\nRUN apt-get install -y curl\n' // tf-allow: 示例输入

export default function DockerfileLint() {
  const { t, errorMessage } = usePlugin()
  const [content, setContent] = useState(SAMPLE) // tf-allow: 默认值
  const lint = useDebouncedCall<LintOut>(content.trim() ? 'lint' : null, { content }, [content])

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<Container />} title={t('ui.input')} bodyClassName="h-full p-3">
        <textarea
          value={content}
          onChange={(event) => setContent(event.target.value)}
          spellCheck={false}
          aria-label={t('ui.input')}
          className="h-full w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
      </Panel>
      <Panel
        icon={<Container />}
        title={t('ui.issues', { count: lint.result?.issues.length ?? 0 })}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-1.5 p-3"
        extra={lint.pending && <Spinner className="size-3.5" />}
      >
        {lint.error ? (
          <p className="text-xs text-danger">{errorMessage(lint.error)}</p>
        ) : (
          lint.result && (
            <>
              <Badge variant="neutral">{t('ui.stages', { count: lint.result.stages })}</Badge>
              {lint.result.issues.length === 0 && <p className="text-xs text-success">{t('ui.clean')}</p>}
              {lint.result.issues.map((issue, index) => (
                <div key={index} className="rounded-control border border-border p-2">
                  <div className="flex items-center gap-2">
                    <Badge variant={issue.severity === 'error' ? 'danger' : issue.severity === 'warning' ? 'warning' : 'neutral'}>
                      {t(`severity.${issue.severity}`)}
                    </Badge>
                    <span className="text-xs font-medium text-fg">{t(`rules.${issue.rule}`)}</span>
                    {issue.line > 0 && <span className="ml-auto text-[11px] text-fg-subtle">{t('ui.line', { line: issue.line })}</span>}
                  </div>
                  <p className="mt-1 rounded-control bg-surface-2 p-1.5 font-mono text-[11px] text-fg-muted">{issue.fix}</p>
                </div>
              ))}
            </>
          )
        )}
      </Panel>
    </div>
  )
}
