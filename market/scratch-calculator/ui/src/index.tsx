import { useState } from 'react'
import { CodeEditor, Empty, Panel, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Calculator, HelpCircle } from 'lucide-react'

interface EvalLine {
  raw: string
  name: string | null
  value: number | null
  error: string | null
}

interface EvalResult {
  lines: EvalLine[]
  last: number | null
}

const DEFAULT_INPUT = 'x = 6\nx * 7\n1km + 10cm\nprev * 2' // tf-allow: 默认输入值

export default function ScratchCalculator() {
  const { t } = usePlugin()
  const [input, setInput] = useState(DEFAULT_INPUT) // tf-allow: 默认输入值
  useLaunchInput((text) => setInput(text))

  const trimmed = input.trim()
  const { result, pending } = useDebouncedCall<EvalResult>(
    trimmed ? 'evaluate' : null,
    { input },
    [input],
  )
  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<Calculator />} title={t('input.label')} className="min-h-0 overflow-hidden" bodyClassName="h-full">
        <CodeEditor
          value={input}
          onChange={setInput}
          language="text"
          lineWrapping
          placeholder={t('input.placeholder')}
          aria-label={t('input.label')}
          className="h-full"
        />
      </Panel>
      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel
          icon={<Calculator />}
          title={t('results.title')}
          className="min-h-0 overflow-hidden"
          bodyClassName="min-h-0 h-full overflow-auto py-1"
          extra={pending && <Spinner className="size-3.5" />}
        >
          {result && result.lines.length > 0 ? (
            result.lines.map((line, index) => (
              <ValueRow
                key={index}
                label={line.name ? `${line.raw} (${t('results.assigned')})` : line.raw}
                value={line.error ? t(`errors.${line.error}`) : format(line.value ?? 0)}
                labelWidth="w-40"
              />
            ))
          ) : (
            <Empty icon={<Calculator />} title={t('results.empty')} />
          )}
        </Panel>
        <Panel icon={<HelpCircle />} title="Syntax" className="shrink-0" bodyClassName="p-3">
          <ul className="list-inside list-disc space-y-1 text-xs text-fg-muted">
            {(t('help.syntax', { returnObjects: true }) as string[]).map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </Panel>
      </div>
    </div>
  )
}

function format(value: number): string {
  if (Number.isInteger(value) && Math.abs(value) < 1e15) return String(value)
  if (value !== 0 && (Math.abs(value) >= 1e12 || Math.abs(value) < 1e-6)) {
    return value.toExponential(6)
  }
  return String(Number(value.toPrecision(12)))
}
