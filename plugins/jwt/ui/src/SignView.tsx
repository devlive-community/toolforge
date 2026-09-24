import { useState } from 'react'
import { Button, CodeEditor, Panel, Select, SegmentedControl, Spinner } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Braces, FileKey2, KeyRound, ScanSearch } from 'lucide-react'
import { ALGORITHMS, isHmac, type Algorithm, type KeyEncoding } from './types'

const DEFAULT_PAYLOAD = `{
  "sub": "1234567890",
  "name": "ToolForge",
  "role": "admin",
  "iat": 1790000000,
  "exp": 1890000000
}`

export function SignView({ onOpen }: { onOpen: (token: string) => void }) {
  const { t, errorMessage } = usePlugin()
  const [algorithm, setAlgorithm] = useState<Algorithm>('HS256')
  const [header, setHeader] = useState('{\n  "kid": "key-1"\n}')
  const [payload, setPayload] = useState(DEFAULT_PAYLOAD)
  const [key, setKey] = useState('your-256-bit-secret')
  const [keyEncoding, setKeyEncoding] = useState<KeyEncoding>('text')

  const ready = payload.trim().length > 0 && key.trim().length > 0
  const { result, error, pending } = useDebouncedCall<{ token: string }>(
    ready ? 'sign' : null,
    ready ? { algorithm, header, payload, key, keyEncoding } : null,
    [algorithm, header, payload, key, keyEncoding],
  )

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[auto_minmax(0,0.5fr)_minmax(0,1fr)_minmax(0,0.7fr)] gap-3">
        <section className="flex items-center gap-3 rounded-card border border-border bg-surface px-3 py-2.5 shadow-card">
          <span className="text-[13px] text-fg-muted">{t('sign.algorithm')}</span>
          <Select<Algorithm>
            value={algorithm}
            onValueChange={setAlgorithm}
            className="w-36"
            aria-label={t('sign.algorithm')}
            options={ALGORITHMS.map((value) => ({ value, label: value }))}
          />
          {isHmac(algorithm) && (
            <SegmentedControl<KeyEncoding>
              size="sm"
              value={keyEncoding}
              onValueChange={setKeyEncoding}
              aria-label={t('verify.encoding')}
              options={[
                { value: 'text', label: t('verify.text') },
                { value: 'base64', label: 'Base64' },
              ]}
            />
          )}
        </section>
        <Panel icon={<Braces />} title={t('sign.header')}>
          <CodeEditor value={header} onChange={setHeader} language="json" aria-label={t('sign.header')} />
        </Panel>
        <Panel icon={<Braces />} title={t('sign.payload')}>
          <CodeEditor value={payload} onChange={setPayload} language="json" aria-label={t('sign.payload')} />
        </Panel>
        <Panel icon={<FileKey2 />} title={t(isHmac(algorithm) ? 'sign.secret' : 'sign.privateKey')}>
          <CodeEditor
            value={key}
            onChange={setKey}
            language="text"
            lineWrapping
            placeholder={t(isHmac(algorithm) ? 'verify.secretPlaceholder' : 'sign.privatePlaceholder')}
            aria-label={t('sign.secret')}
          />
        </Panel>
      </div>

      <Panel
        icon={<KeyRound />}
        title={t('sign.result')}
        extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
        actions={
          result &&
          !error && (
            <>
              <Button size="sm" onClick={() => onOpen(result.token)}>
                <ScanSearch />
                {t('sign.open')}
              </Button>
              <CopyButton text={result.token} label={t('common:copy')} variant="outline" />
            </>
          )
        }
      >
        {error ? (
          <p className="p-4 text-[13px] text-danger">
            {errorMessage(error)}
            {typeof error.params?.detail === 'string' && <span className="mt-1 block font-mono text-xs text-fg-muted">{error.params.detail}</span>}
          </p>
        ) : (
          <CodeEditor value={result?.token ?? ''} readOnly language="text" lineWrapping aria-label={t('sign.result')} />
        )}
      </Panel>
    </div>
  )
}
