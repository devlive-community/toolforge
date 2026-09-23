import { useState } from 'react'
import { CodeEditor, Empty, Panel, Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { FileText, Fingerprint } from 'lucide-react'
import { DigestRow } from './components/DigestRow'
import { formatBytes } from './format'
import { ALGORITHM_LABELS, type Algorithm, type TextReport } from './types'

export function TextHash({ algorithms, uppercase }: { algorithms: Algorithm[]; uppercase: boolean }) {
  const { t, errorMessage } = usePlugin()
  const [input, setInput] = useState('ToolForge')
  const args = algorithms.length > 0 ? { input, algorithms, uppercase } : null
  const { result, error, pending } = useDebouncedCall<TextReport>(args ? 'hash_text' : null, args, [input, algorithms.join(), uppercase])

  return (
    <div className="grid h-full grid-cols-2 gap-3">
      <Panel
        icon={<FileText />}
        title={t('text.input')}
        footer={result && <span>{t('text.bytes', { size: formatBytes(result.bytes) })}</span>}
      >
        <CodeEditor value={input} onChange={setInput} language="text" lineWrapping placeholder={t('text.placeholder')} aria-label={t('text.input')} />
      </Panel>
      <Panel
        icon={<Fingerprint />}
        title={t('text.result')}
        extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
        footer={result && <span>{t('elapsed', { ms: result.elapsedMs })}</span>}
        bodyClassName="overflow-auto p-2"
      >
        {error ? (
          <p className="p-4 text-[13px] text-danger">{errorMessage(error)}</p>
        ) : algorithms.length === 0 ? (
          <Empty icon={<Fingerprint />} title={t('noAlgorithms')} />
        ) : (
          result?.results.map((item) => <DigestRow key={item.algorithm} label={ALGORITHM_LABELS[item.algorithm]} digest={item.digest} />)
        )}
      </Panel>
    </div>
  )
}
