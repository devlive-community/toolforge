import { useState } from 'react'
import { CodeEditor, Input, Panel, Select, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Coins, FileDigit } from 'lucide-react'

interface Cost {
  input: number
  output: number
  total: number
}

interface CountOut {
  model: string
  method: string
  tokens: number
  characters: number
  charactersNoSpaces: number
  cjkChars: number
  latinWords: number
  digitGroups: number
  contextWindow: number
  windowUsage: number
  cost: Cost | null
}

const MODELS = [
  'gpt-4o', 'gpt-4o-mini', 'gpt-4.1', 'o3', 'claude-sonnet', 'claude-haiku', 'gemini-pro', 'deepseek-chat', 'qwen-max', 'generic',
]

const DEFAULT_TEXT = '把要计量的文本粘贴到这里。Token 数量随模型不同而略有差异，结果为启发式近似值。' // tf-allow: 默认输入值

interface SavedPrices {
  model: string
  inputPrice: number | undefined
  outputPrice: number | undefined
}

export default function TokenCounter() {
  const { t } = usePlugin()
  const [text, setText] = useState(DEFAULT_TEXT) // tf-allow: 默认输入值
  // 本地保存的单价与模型；未保存过 overrides 时显示已存值
  const [overrides, setOverrides] = useState<SavedPrices | null>(null)
  const [saved, savePrices] = usePluginState<SavedPrices>('prices', { model: 'gpt-4o', inputPrice: undefined, outputPrice: undefined })
  const [expectedOutput, setExpectedOutput] = useState<number | undefined>()

  const model = overrides?.model ?? saved?.model ?? 'gpt-4o'
  const inputPrice = overrides?.inputPrice ?? saved?.inputPrice
  const outputPrice = overrides?.outputPrice ?? saved?.outputPrice
  const setModel = (value: string) => setOverrides((prev) => ({ model: value, inputPrice: prev?.inputPrice ?? saved?.inputPrice, outputPrice: prev?.outputPrice ?? saved?.outputPrice }))
  const setInputPrice = (value: number | undefined) => setOverrides((prev) => ({ model: prev?.model ?? saved?.model ?? 'gpt-4o', inputPrice: value, outputPrice: prev?.outputPrice ?? saved?.outputPrice }))
  const setOutputPrice = (value: number | undefined) => setOverrides((prev) => ({ model: prev?.model ?? saved?.model ?? 'gpt-4o', inputPrice: prev?.inputPrice ?? saved?.inputPrice, outputPrice: value }))

  const trimmed = text.trim()
  const args = trimmed
    ? { text, model, inputPrice, outputPrice, expectedOutput }
    : null
  const { result, pending } = useDebouncedCall<CountOut>(args ? 'count' : null, args, [text, model, inputPrice, outputPrice, expectedOutput])

  const remember = () => {
    const prices: SavedPrices = { model, inputPrice, outputPrice }
    savePrices(prices)
    setOverrides(prices)
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-2 gap-3">
      <Panel icon={<FileDigit />} title={t('results.input')} className="min-h-0 overflow-hidden" bodyClassName="h-full space-y-2 p-3">
        <div className="grid shrink-0 grid-cols-2 gap-2">
          <Select
            value={model}
            onValueChange={setModel}
            options={MODELS.map((key) => ({ value: key, label: t(`models.${key}`) }))}
            aria-label={t('api.count.model')}
          />
          <Input
            type="number"
            value={expectedOutput === undefined ? '' : String(expectedOutput)}
            onChange={(event) => setExpectedOutput(event.target.value === '' ? undefined : Number(event.target.value))}
            min={0}
            placeholder={t('cost.expectedOutput')}
            aria-label={t('cost.expectedOutput')}
          />
          <Input
            type="number"
            value={inputPrice === undefined ? '' : String(inputPrice)}
            onChange={(event) => setInputPrice(event.target.value === '' ? undefined : Number(event.target.value))}
            min={0}
            placeholder={t('cost.inputPrice')}
            aria-label={t('cost.inputPrice')}
          />
          <Input
            type="number"
            value={outputPrice === undefined ? '' : String(outputPrice)}
            onChange={(event) => setOutputPrice(event.target.value === '' ? undefined : Number(event.target.value))}
            min={0}
            placeholder={t('cost.outputPrice')}
            aria-label={t('cost.outputPrice')}
          />
        </div>
        <button
          type="button"
          onClick={remember}
          className="w-full shrink-0 rounded-control border border-border px-2 py-1.5 text-xs text-fg-muted outline-none transition-colors hover:bg-hover hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
        >
          {t('cost.remember')}
        </button>
        <div className="min-h-0 flex-1">
          <CodeEditor
            value={text}
            onChange={setText}
            language="text"
            lineWrapping
            aria-label={t('api.count.text')}
            className="h-full"
          />
        </div>
      </Panel>
      <Panel
        icon={<Coins />}
        title={t('results.title')}
        className="min-h-0"
        bodyClassName="min-h-0 overflow-auto space-y-1 py-1"
        extra={pending && <Spinner className="size-3.5" />}
      >
        {result ? (
          <div className="space-y-1 px-3 py-2">
            <ValueRow label={t('results.tokens')} value={String(result.tokens)} labelWidth="w-36" />
            <p className="pl-[calc(9rem+0.5rem)] text-[11px] text-fg-subtle">{t(`method.${result.method}`)}</p>
            <ValueRow label={t('results.characters')} value={`${result.characters} (${t('results.charsNoSpaces')}: ${result.charactersNoSpaces})`} labelWidth="w-36" />
            <ValueRow label={t('results.cjk')} value={String(result.cjkChars)} labelWidth="w-36" />
            <ValueRow label={t('results.latinWords')} value={String(result.latinWords)} labelWidth="w-36" />
            <ValueRow label={t('results.digitGroups')} value={String(result.digitGroups)} labelWidth="w-36" />
            <ValueRow
              label={t('results.usage')}
              value={`${(result.windowUsage * 100).toFixed(1)}% · ${t('results.window')} ${result.contextWindow.toLocaleString()}`}
              labelWidth="w-36"
            />
            {result.cost && (
              <>
                <div className="my-1 h-px bg-border" />
                <ValueRow label={t('cost.input')} value={result.cost.input.toFixed(6)} labelWidth="w-36" />
                <ValueRow label={t('cost.output')} value={result.cost.output.toFixed(6)} labelWidth="w-36" />
                <ValueRow label={t('cost.total')} value={result.cost.total.toFixed(6)} labelWidth="w-36" />
              </>
            )}
          </div>
        ) : (
          <p className="py-6 text-center text-xs text-fg-subtle">{t('results.empty')}</p>
        )}
      </Panel>
    </div>
  )
}
