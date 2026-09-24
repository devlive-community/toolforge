import { useState } from 'react'
import { Badge, Button, Checkbox, CodeEditor, Empty, Panel, SegmentedControl, Select, Spinner, Tooltip, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowLeftRight, CircleCheck, FileText, GitCompare } from 'lucide-react'
import { SideBySide } from './SideBySide'
import { LEFT_SAMPLE, RIGHT_SAMPLE, type Algo, type DiffResult, type Mode } from './types'

const tokenStyle = {
  equal: '',
  delete: 'rounded-[3px] bg-danger-soft text-danger line-through decoration-danger/60',
  insert: 'rounded-[3px] bg-success-soft text-success',
  change: '',
}

export function DiffTool() {
  const { t, errorMessage } = usePlugin()
  const [left, setLeft] = useState(LEFT_SAMPLE)
  const [right, setRight] = useState(RIGHT_SAMPLE)
  const [mode, setMode] = useState<Mode>('lines')
  const [algorithm, setAlgorithm] = useState<Algo>('myers')
  const [ignoreCase, setIgnoreCase] = useState(false)
  const [ignoreWhitespace, setIgnoreWhitespace] = useState(false)

  const { result, error, pending } = useDebouncedCall<DiffResult>(
    'diff',
    { left, right, mode, algorithm, ignoreCase, ignoreWhitespace },
    [left, right, mode, algorithm, ignoreCase, ignoreWhitespace],
  )
  const stats = result?.stats

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <section className="flex shrink-0 flex-wrap items-center gap-3 rounded-card border border-border bg-surface px-3 py-2 shadow-card">
        <SegmentedControl<Mode>
          value={mode}
          onValueChange={setMode}
          aria-label={t('mode.label')}
          options={[
            { value: 'lines', label: t('mode.lines') },
            { value: 'words', label: t('mode.words') },
            { value: 'chars', label: t('mode.chars') },
          ]}
        />
        {mode === 'lines' && (
          <>
            <Checkbox checked={ignoreCase} onCheckedChange={setIgnoreCase}>
              {t('options.ignoreCase')}
            </Checkbox>
            <Checkbox checked={ignoreWhitespace} onCheckedChange={setIgnoreWhitespace}>
              {t('options.ignoreWhitespace')}
            </Checkbox>
          </>
        )}
        <Select<Algo>
          size="sm"
          value={algorithm}
          onValueChange={setAlgorithm}
          aria-label={t('options.algorithm')}
          options={[
            { value: 'myers', label: 'Myers' },
            { value: 'patience', label: 'Patience' },
          ]}
        />
        <Tooltip content={t('swap')}>
          <Button
            size="icon-md"
            aria-label={t('swap')}
            onClick={() => {
              setLeft(right)
              setRight(left)
            }}
          >
            <ArrowLeftRight />
          </Button>
        </Tooltip>
        <div className="ml-auto flex items-center gap-2">
          {pending && <Spinner className="size-3.5 text-fg-subtle" />}
          {stats && (
            <>
              <Badge variant="success">+{stats.added}</Badge>
              <Badge variant="danger">−{stats.removed}</Badge>
              {mode === 'lines' && <Badge variant="warning">~{stats.changed}</Badge>}
              <Badge>{t('similarity', { value: Math.round(stats.similarity * 100) })}</Badge>
            </>
          )}
          {result && !result.identical && <CopyButton text={result.unified} label={t('copyPatch')} variant="outline" />}
        </div>
      </section>

      <div className="grid min-h-0 flex-[0.8] grid-cols-2 gap-3">
        <Panel icon={<FileText />} title={t('left')}>
          <CodeEditor value={left} onChange={setLeft} language="text" aria-label={t('left')} />
        </Panel>
        <Panel icon={<FileText />} title={t('right')}>
          <CodeEditor value={right} onChange={setRight} language="text" aria-label={t('right')} />
        </Panel>
      </div>

      <Panel
        className="min-h-0 flex-[1.2]"
        icon={<GitCompare />}
        title={t('result')}
        extra={result?.truncated && <Badge variant="warning">{t('truncated')}</Badge>}
        footer={result && <span>{t('elapsed', { ms: result.elapsedMs })}</span>}
      >
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : !result ? (
          <div className="flex h-full items-center justify-center text-fg-muted">
            <Spinner />
          </div>
        ) : result.identical ? (
          <Empty icon={<CircleCheck className="text-success" />} title={t('identical')} />
        ) : result.mode === 'lines' ? (
          <SideBySide rows={result.rows} />
        ) : (
          <div className="h-full overflow-auto p-4 font-mono text-[13px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
            {result.tokens.map((token, index) => (
              <span key={index} className={cn(tokenStyle[token.tag])}>
                {token.text}
              </span>
            ))}
          </div>
        )}
      </Panel>
    </div>
  )
}
