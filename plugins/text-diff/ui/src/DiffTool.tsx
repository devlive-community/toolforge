import { useState } from 'react'
import { Badge, Button, Checkbox, CodeEditor, Empty, Panel, SegmentedControl, Select, Spinner, Tooltip, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { ArrowLeftRight, CircleCheck, EyeOff, FileText, GitCompare } from 'lucide-react'
import { IgnoreRules, type IgnoreRule } from './IgnoreRules'
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
  const [rules, setRules] = usePluginState<IgnoreRule[]>('ignoreRules', [])
  const [showRules, setShowRules] = useState(false)
  // 只发送启用的规则；保留原序号以便定位无效规则
  const active = rules.map((rule, index) => ({ ...rule, index })).filter((rule) => rule.enabled && rule.pattern)
  const ignorePatterns = active.map((rule) => rule.pattern)

  const { result, error, pending } = useDebouncedCall<DiffResult>(
    'diff',
    { left, right, mode, algorithm, ignoreCase, ignoreWhitespace, ignorePatterns },
    [left, right, mode, algorithm, ignoreCase, ignoreWhitespace, ignorePatterns.join('\u0000')],
  )
  const invalidIndex = error?.code === 'diff.invalid_pattern' ? (active[Number(error.params?.index ?? 0) - 1]?.index ?? null) : null
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
            <Button size="sm" variant={showRules ? 'soft' : 'outline'} aria-pressed={showRules} onClick={() => setShowRules((v) => !v)}>
              <EyeOff />
              {t('ignore.toggle')}
              {active.length > 0 && <Badge variant="primary">{active.length}</Badge>}
            </Button>
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
              {mode === 'lines' && stats.ignored > 0 && <Badge variant="info">{t('ignoredLines', { count: stats.ignored })}</Badge>}
              <Badge>{t('similarity', { value: Math.round(stats.similarity * 100) })}</Badge>
            </>
          )}
          {result && !result.identical && <CopyButton text={result.unified} label={t('copyPatch')} variant="outline" />}
        </div>
      </section>

      {mode === 'lines' && showRules && <IgnoreRules rules={rules} onChange={setRules} invalidIndex={invalidIndex} />}

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
          <div className="p-4 text-[13px]">
            <p className="text-danger">{errorMessage(error)}</p>
            {typeof error.params?.detail === 'string' && (
              <p className="mt-2 rounded-control bg-surface-2 px-2.5 py-1.5 font-mono text-xs break-all whitespace-pre-wrap text-fg-muted" data-selectable>
                {error.params.detail}
              </p>
            )}
          </div>
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
