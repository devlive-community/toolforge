import { useState } from 'react'
import { Badge, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { GitBranch } from 'lucide-react'

interface Job {
  id: string
  needs: string[]
  steps: number
  condition: string | null
  level: number
}
interface ParseOut {
  name: string | null
  triggers: string[]
  jobs: Job[]
  cycles: string[]
  edges: number
  mermaid: string
}

const SAMPLE = 'name: CI\non:\n  push:\njobs:\n  build:\n    steps: [{run: cargo build}]\n  test:\n    needs: build\n    steps: [{run: cargo test}]\n  deploy:\n    needs: test\n    steps: [{run: echo deploy}]' // tf-allow: 示例输入

export default function WorkflowDag() {
  const { t, errorMessage } = usePlugin()
  const [yaml, setYaml] = useState(SAMPLE) // tf-allow: 默认值
  const parse = useDebouncedCall<ParseOut>(yaml.trim() ? 'parse' : null, { yaml }, [yaml])

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<GitBranch />} title={t('ui.input')} bodyClassName="p-3">
        <textarea
          value={yaml}
          onChange={(event) => setYaml(event.target.value)}
          spellCheck={false}
          aria-label={t('ui.input')}
          className="h-28 w-full resize-none rounded-control border border-border bg-surface p-2 font-mono text-xs text-fg outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
      </Panel>
      <Panel icon={<GitBranch />} title={t('ui.jobs', { count: parse.result?.jobs.length ?? 0 })} className="min-h-0 flex-1" bodyClassName="min-h-0 overflow-auto space-y-1 p-3" extra={parse.pending && <Spinner className="size-3.5" />}>
        {parse.error ? (
          <p className="text-xs text-danger">{errorMessage(parse.error)}</p>
        ) : (
          parse.result && (
            <>
              {parse.result.triggers.length > 0 && (
                <div className="flex flex-wrap gap-1.5">
                  {parse.result.triggers.map((trigger) => (
                    <Badge key={trigger} variant="neutral">{trigger}</Badge>
                  ))}
                </div>
              )}
              {parse.result.cycles.length > 0 ? (
                <div className="rounded-control bg-danger-soft p-2 text-xs text-danger">
                  <p className="font-medium">{t('ui.cycles')}</p>
                  {parse.result.cycles.map((cycle) => <p key={cycle}>{cycle}</p>)}
                </div>
              ) : (
                <Badge variant="success">{t('ui.noCycles')}</Badge>
              )}
              {parse.result.jobs.map((job) => (
                <div key={job.id} className="border-t border-border py-1.5">
                  <p className="text-xs font-medium text-fg">
                    {job.id}
                    <span className="ml-1.5 text-fg-subtle">{t('ui.level', { level: job.level })} · {t('ui.steps', { count: job.steps })}</span>
                  </p>
                  {job.needs.length > 0 && <p className="text-[11px] text-fg-muted">{t('ui.needs')}: {job.needs.join(', ')}</p>}
                  {job.condition && <p className="truncate font-mono text-[11px] text-fg-subtle">{t('ui.condition')}: {job.condition}</p>}
                </div>
              ))}
            </>
          )
        )}
      </Panel>
    </div>
  )
}
