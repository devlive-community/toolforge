import { useState } from 'react'
import { Badge, CodeEditor, Empty, Panel, SegmentedControl, Spinner } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Binary, Braces, CircleX, FileKey2, KeyRound, ShieldQuestion } from 'lucide-react'
import { SAMPLE_JWE, SAMPLE_JWE_KEY_HEX, type Decrypted, type JweKeyEncoding, type ParsedJwe } from './types'

type JwePartKey = 'jwe.partHeader' | 'jwe.partKey' | 'jwe.partIv' | 'jwe.partCiphertext' | 'jwe.partTag'
const PART_KEYS: JwePartKey[] = ['jwe.partHeader', 'jwe.partKey', 'jwe.partIv', 'jwe.partCiphertext', 'jwe.partTag']

export function JweView() {
  const { t, errorMessage } = usePlugin()
  const [token, setToken] = useState(SAMPLE_JWE)
  const [keyText, setKeyText] = useState(SAMPLE_JWE_KEY_HEX)
  const [keyEncoding, setKeyEncoding] = useState<JweKeyEncoding>('hex')
  const [privatePem, setPrivatePem] = useState('')

  const trimmed = token.trim()
  // 仅解析（无需密钥）：protected header、五段长度与算法组合
  const parsed = useDebouncedCall<ParsedJwe>(trimmed ? 'parse_jwe' : null, trimmed ? { token } : null, [token])
  const parsedResult = parsed.error ? null : parsed.result
  const isRsa = parsedResult?.alg === 'RSA-OAEP-256'

  const keyReady = !!parsedResult && (isRsa ? privatePem.trim() : keyText.trim()).length > 0
  const decrypted = useDebouncedCall<Decrypted>(
    keyReady ? 'decrypt_jwe' : null,
    keyReady ? { token, keyText, keyEncoding, privatePem } : null,
    [token, keyText, keyEncoding, privatePem],
  )
  const result = decrypted.error ? null : decrypted.result

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[minmax(0,1.2fr)_minmax(0,1fr)] gap-3">
        <Panel
          icon={<KeyRound />}
          title={t('jwe.token')}
          extra={(parsed.pending || decrypted.pending) && <Spinner className="size-3.5 text-fg-subtle" />}
        >
          <CodeEditor value={token} onChange={setToken} language="text" lineWrapping placeholder={t('jwe.placeholder')} aria-label={t('jwe.token')} />
        </Panel>

        <Panel
          icon={<FileKey2 />}
          title={t(isRsa ? 'sign.privateKey' : 'jwe.keyTitle')}
          actions={
            !isRsa && (
              <SegmentedControl<JweKeyEncoding>
                size="sm"
                value={keyEncoding}
                onValueChange={setKeyEncoding}
                aria-label={t('jwe.keyEncoding')}
                options={[
                  { value: 'hex', label: 'Hex' },
                  { value: 'base64url', label: 'Base64URL' },
                ]}
              />
            )
          }
          footer={
            <span className={decrypted.error ? 'text-danger' : undefined}>
              {decrypted.error
                ? errorMessage(decrypted.error)
                : t(isRsa ? 'jwe.hintPem' : 'jwe.hintKey')}
            </span>
          }
        >
          {isRsa ? (
            <CodeEditor value={privatePem} onChange={setPrivatePem} language="text" lineWrapping placeholder={t('sign.privatePlaceholder')} aria-label={t('sign.privateKey')} />
          ) : (
            <CodeEditor value={keyText} onChange={setKeyText} language="text" lineWrapping placeholder={t('jwe.keyPlaceholder')} aria-label={t('jwe.keyTitle')} />
          )}
        </Panel>
      </div>

      {!trimmed ? (
        <Panel>
          <Empty icon={<ShieldQuestion />} title={t('jwe.empty')} description={t('jwe.emptyHint')} />
        </Panel>
      ) : parsed.error ? (
        <Panel>
          <Empty icon={<CircleX />} title={errorMessage(parsed.error)} />
        </Panel>
      ) : (
        <div className="grid min-h-0 grid-rows-[auto_minmax(0,0.7fr)_minmax(0,1fr)] gap-3">
          <section className="flex flex-wrap items-center gap-1.5 rounded-card border border-border bg-surface px-3 py-2.5 shadow-card">
            {parsedResult?.alg && <Badge variant="info">{parsedResult.alg}</Badge>}
            {parsedResult?.enc && <Badge variant="primary">{parsedResult.enc}</Badge>}
            {parsedResult && (
              <>
                <Badge>
                  {t('jwe.encryptedKeyLen')}: {parsedResult.encryptedKeyLen}
                </Badge>
                {PART_KEYS.map((part, i) => (
                  <Badge key={part}>
                    {t(part)} <span className="font-mono">{parsedResult.parts[i]}</span>
                  </Badge>
                ))}
              </>
            )}
            {result?.kind === 'binary' && (
              <Badge variant="warning">
                <Binary className="size-3" />
                {t('jwe.kindBinary')}
              </Badge>
            )}
          </section>
          <Panel icon={<Braces />} title={t('decode.header')}>
            <CodeEditor value={parsedResult ? JSON.stringify(parsedResult.header, null, 2) : ''} readOnly language="json" lineWrapping aria-label={t('decode.header')} />
          </Panel>
          <Panel
            icon={<Braces />}
            title={t('jwe.plaintext')}
            actions={
              result &&
              result.kind !== 'binary' && <CopyButton text={result.plaintext} label={t('jwe.plaintext')} variant="outline" />
            }
          >
            {decrypted.error ? (
              <p className="p-4 text-[13px] text-danger">{errorMessage(decrypted.error)}</p>
            ) : result?.kind === 'binary' ? (
              <CodeEditor value={result.plaintextBase64} readOnly language="text" lineWrapping aria-label={t('jwe.plaintext')} />
            ) : (
              <CodeEditor
                value={result?.plaintext ?? ''}
                readOnly
                language={result?.kind === 'json' ? 'json' : 'text'}
                lineWrapping
                aria-label={t('jwe.plaintext')}
              />
            )}
          </Panel>
        </div>
      )}
    </div>
  )
}
