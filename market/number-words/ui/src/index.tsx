import { Badge, Button, Empty, Input, Panel, cn } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin, usePluginState, type AppError } from '@toolforge/plugin-ui-sdk'
import { Hash, Sigma } from 'lucide-react'

interface Output {
  kind: string
  value: string | null
  note: AppError | null
}
interface Converted {
  source: 'arabic' | 'chinese' | 'rmb' | 'roman'
  value: string
  outputs: Output[]
}

const EXAMPLES = ['1234567.89', '100010000', '0.05', '壹仟零肆元伍角', '一亿二千万', 'MCMXCIV', '1.5万', '6.02e23']
/** 文字类结果用正文字体，数字格式用等宽字体 */
const WORDS = new Set(['rmb', 'chineseLower', 'chineseUpper', 'english', 'check'])
const PROMINENT = new Set(['rmb'])

function NumberWords() {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = usePluginState('input', '1234567.89')
  const result = useDebouncedCall<Converted>(input.trim() ? 'convert' : null, { input }, [input])
  const r = result.result

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Panel icon={<Hash />} title={t('input.title')} className="shrink-0" bodyClassName="space-y-2.5 p-3">
        <Input
          size="lg"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder={t('input.placeholder')}
          className="text-base"
          aria-label={t('input.title')}
          autoFocus
        />
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-[11px] text-fg-subtle">{t('input.examples')}</span>
          {EXAMPLES.map((example) => (
            <Button key={example} size="sm" variant="ghost" className="h-6 px-2 text-xs" onClick={() => setInput(example)}>
              {example}
            </Button>
          ))}
        </div>
      </Panel>

      <Panel
        icon={<Sigma />}
        title={t('result.title')}
        extra={
          r && (
            <>
              <Badge variant="info">{t(`sources.${r.source}`)}</Badge>
              <span className="font-mono text-xs text-fg-muted tabular-nums">{r.value}</span>
            </>
          )
        }
        className="min-h-0 flex-1"
        bodyClassName="overflow-auto p-2"
      >
        {!input.trim() ? (
          <Empty icon={<Sigma />} title={t('result.empty')} description={t('result.emptyHint')} />
        ) : result.error ? (
          <p className="p-2 text-[13px] text-danger">{errorMessage(result.error)}</p>
        ) : (
          <div className={cn('divide-y divide-border', result.pending && 'opacity-60')}>
            {r?.outputs.map((o) => (
              <div key={o.kind} className="group flex items-start gap-3 px-3 py-2.5 hover:bg-hover">
                <span className="w-32 shrink-0 pt-0.5 text-xs font-medium text-fg-muted">{t(`outputs.${o.kind}`)}</span>
                <div className="min-w-0 flex-1">
                  {o.value !== null ? (
                    <p
                      className={cn(
                        'break-all text-fg',
                        WORDS.has(o.kind) ? 'text-[14px] leading-relaxed' : 'font-mono text-[13px] tabular-nums',
                        PROMINENT.has(o.kind) && 'text-[17px] font-semibold',
                      )}
                      data-selectable
                    >
                      {o.value}
                    </p>
                  ) : (
                    <p className="text-[13px] text-fg-subtle">{t('result.none')}</p>
                  )}
                  {o.note && <p className={cn('mt-0.5 text-[11px]', o.value !== null ? 'text-warning' : 'text-fg-subtle')}>{errorMessage(o.note)}</p>}
                </div>
                {o.value !== null && <CopyButton text={o.value} className="-my-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />}
              </div>
            ))}
          </div>
        )}
      </Panel>
    </div>
  )
}

export default NumberWords
