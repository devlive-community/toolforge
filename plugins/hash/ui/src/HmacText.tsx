import { useState } from 'react'
import { CodeEditor, Empty, Input, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { CircleCheck, CircleX, FileText, KeyRound, ShieldCheck } from 'lucide-react'
import { ALGORITHM_LABELS, type Algorithm, type HmacReport, type KeyEncoding, type OutputEncoding } from './types'

export function HmacText({ algorithms, uppercase }: { algorithms: Algorithm[]; uppercase: boolean }) {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('ToolForge')
  const [key, setKey] = useState('secret')
  const [keyEncoding, setKeyEncoding] = useState<KeyEncoding>('text')
  const [output, setOutput] = useState<OutputEncoding>('hex')
  const [expected, setExpected] = useState('')
  const usable = algorithms.filter((a) => a !== 'crc32')
  const args = usable.length > 0 ? { input, key, keyEncoding, algorithms: usable, output, uppercase, expected } : null
  const { result, error, pending } = useDebouncedCall<HmacReport>(args ? 'hmac_text' : null, args, [input, key, keyEncoding, usable.join(), output, uppercase, expected])
  const keyError = error?.code === 'hmac.invalid_key'

  return (
    <div className="grid h-full grid-cols-2 gap-3">
      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-3">
        <Panel icon={<KeyRound />} title={t('hmac.key')} bodyClassName="space-y-2 p-3">
          <div className="flex items-center gap-2">
            <Input
              value={key}
              onChange={(event) => setKey(event.target.value)}
              className="font-mono"
              wrapperClassName="flex-1"
              invalid={keyError}
              spellCheck={false}
              placeholder={t('hmac.keyPlaceholder')}
              aria-label={t('hmac.key')}
            />
            <SegmentedControl<KeyEncoding>
              size="sm"
              value={keyEncoding}
              onValueChange={setKeyEncoding}
              aria-label={t('hmac.keyEncoding')}
              options={[
                { value: 'text', label: t('hmac.encodings.text') },
                { value: 'hex', label: 'Hex' },
                { value: 'base64', label: 'Base64' },
              ]}
            />
          </div>
          {keyError && error ? (
            <p className="text-xs text-danger">{errorMessage(error)}</p>
          ) : (
            result && <p className="text-xs text-fg-subtle">{t('hmac.keyBytes', { count: result.keyBytes })}</p>
          )}
        </Panel>
        <Panel icon={<FileText />} title={t('hmac.message')}>
          <CodeEditor value={input} onChange={setInput} language="text" lineWrapping placeholder={t('text.placeholder')} aria-label={t('hmac.message')} />
        </Panel>
      </div>

      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel
          icon={<ShieldCheck />}
          title={t('hmac.result')}
          extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            <SegmentedControl<OutputEncoding>
              size="sm"
              value={output}
              onValueChange={setOutput}
              aria-label={t('hmac.output')}
              options={[
                { value: 'hex', label: 'Hex' },
                { value: 'base64', label: 'Base64' },
              ]}
            />
          }
          footer={result && <span>{t('elapsed', { ms: result.elapsedMs })}</span>}
          bodyClassName="overflow-auto p-2"
        >
          {error && !keyError ? (
            <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
          ) : usable.length === 0 ? (
            <Empty icon={<ShieldCheck />} title={t('hmac.noAlgorithms')} />
          ) : (
            result?.results.map((item) => (
              <div key={item.algorithm} className="flex items-center gap-1">
                <div className="min-w-0 flex-1">
                  <ValueRow label={ALGORITHM_LABELS[item.algorithm]} value={item.mac} />
                </div>
                {item.matches === true && <CircleCheck className="size-4 shrink-0 text-success" aria-label={t('hmac.match')} />}
                {item.matches === false && <CircleX className="size-4 shrink-0 text-fg-subtle" aria-label={t('hmac.mismatch')} />}
              </div>
            ))
          )}
        </Panel>
        <Panel icon={<CircleCheck />} title={t('hmac.verify')} className="shrink-0" bodyClassName="space-y-2 p-3">
          <Input
            value={expected}
            onChange={(event) => setExpected(event.target.value)}
            className="font-mono"
            placeholder={t('hmac.verifyPlaceholder')}
            spellCheck={false}
            aria-label={t('hmac.verify')}
          />
          {expected.trim() && result && (
            <p className={result.results.some((r) => r.matches) ? 'text-xs text-success' : 'text-xs text-warning'}>
              {result.results.some((r) => r.matches)
                ? t('hmac.verified', { algorithm: result.results.filter((r) => r.matches).map((r) => ALGORITHM_LABELS[r.algorithm]).join(', ') })
                : t('hmac.notVerified')}
            </p>
          )}
        </Panel>
      </div>
    </div>
  )
}
