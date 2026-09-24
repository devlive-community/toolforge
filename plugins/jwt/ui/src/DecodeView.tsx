import { useMemo, useState } from 'react'
import { Badge, CodeEditor, Empty, Panel, SegmentedControl, Spinner, type EditorMark } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BadgeCheck, Braces, CircleX, FileKey2, KeyRound, ShieldQuestion } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { formatRelative } from './relative'
import { isHmac, type Decoded, type KeyEncoding, type Verified } from './types'

const localTime = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' })

export function DecodeView({ token, onTokenChange }: { token: string; onTokenChange: (token: string) => void }) {
  const { t, errorMessage } = usePlugin()
  const { i18n } = useTranslation()
  const [key, setKey] = useState('')
  const [keyEncoding, setKeyEncoding] = useState<KeyEncoding>('text')

  const trimmed = token.trim()
  const decoded = useDebouncedCall<Decoded>(trimmed ? 'decode' : null, trimmed ? { token } : null, [token])
  const result = decoded.error ? null : decoded.result
  const canVerify = !!result && key.trim().length > 0
  const verified = useDebouncedCall<Verified>(canVerify ? 'verify' : null, canVerify ? { token, key, keyEncoding } : null, [token, key, keyEncoding])

  const marks = useMemo<EditorMark[]>(() => {
    if (!result) return []
    const { header, payload, signature } = result.segments
    return [
      { from: header[0], to: header[1], tone: 'alt' },
      { from: payload[0], to: payload[1], tone: 'primary' },
      { from: signature[0], to: signature[1], tone: 'warn' },
    ]
  }, [result])

  const status = result?.status
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-3">
        <Panel
          icon={<KeyRound />}
          title={t('decode.token')}
          extra={decoded.pending && <Spinner className="size-3.5 text-fg-subtle" />}
          footer={
            result && (
              <div className="flex flex-wrap items-center gap-1.5">
                <Badge variant="info">{t('decode.segmentHeader')}</Badge>
                <Badge variant="primary">{t('decode.segmentPayload')}</Badge>
                <Badge variant="warning">{t('decode.segmentSignature')}</Badge>
              </div>
            )
          }
        >
          <CodeEditor value={token} onChange={onTokenChange} language="text" lineWrapping marks={marks} placeholder={t('decode.placeholder')} aria-label={t('decode.token')} />
        </Panel>

        <Panel
          icon={<FileKey2 />}
          title={t('verify.title')}
          extra={
            verified.result && !verified.error && canVerify ? (
              verified.result.valid ? (
                <Badge variant="success">
                  <BadgeCheck className="size-3" />
                  {t('verify.valid')}
                </Badge>
              ) : (
                <Badge variant="danger">
                  <CircleX className="size-3" />
                  {t('verify.invalid')}
                </Badge>
              )
            ) : null
          }
          actions={
            isHmac(result?.algorithm) && (
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
            )
          }
          bodyClassName="h-36"
          footer={
            <span className={verified.error || verified.result?.reason ? 'text-danger' : undefined}>
              {verified.error
                ? errorMessage(verified.error)
                : verified.result?.reason
                  ? errorMessage({ code: verified.result.reason })
                  : t(isHmac(result?.algorithm) ? 'verify.hintSecret' : 'verify.hintPublic')}
            </span>
          }
        >
          <CodeEditor value={key} onChange={setKey} language="text" lineWrapping placeholder={t(isHmac(result?.algorithm) ? 'verify.secretPlaceholder' : 'verify.pemPlaceholder')} aria-label={t('verify.title')} />
        </Panel>
      </div>

      {!trimmed ? (
        <Panel>
          <Empty icon={<ShieldQuestion />} title={t('decode.empty')} description={t('decode.emptyHint')} />
        </Panel>
      ) : decoded.error ? (
        <Panel>
          <Empty icon={<CircleX />} title={errorMessage(decoded.error)} />
        </Panel>
      ) : (
        <div className="grid min-h-0 grid-rows-[auto_minmax(0,0.6fr)_minmax(0,1fr)] gap-3">
          <section className="flex flex-wrap items-center gap-2 rounded-card border border-border bg-surface px-3 py-2.5 shadow-card">
            {result?.algorithm && <Badge variant="info">{result.algorithm}</Badge>}
            {status?.expired === true && <Badge variant="danger">{t('status.expired')}</Badge>}
            {status?.expired === false && <Badge variant="success">{t('status.active')}</Badge>}
            {status?.expired === null && <Badge>{t('status.noExpiry')}</Badge>}
            {status?.notYetValid && <Badge variant="warning">{t('status.notYetValid')}</Badge>}
            <dl className="mt-1 grid w-full grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
              {status?.times.map((claim) => (
                <div key={claim.name} className="contents">
                  <dt className="text-fg-muted">
                    {t(`claims.${claim.name}`)} <span className="font-mono text-fg-subtle">{claim.name}</span>
                  </dt>
                  <dd className="font-mono text-fg" data-selectable>
                    {localTime.format(claim.value * 1000)}
                    <span className="ml-2 font-sans text-fg-subtle">{formatRelative(claim.relativeSeconds, i18n.language)}</span>
                  </dd>
                </div>
              ))}
            </dl>
          </section>
          <Panel icon={<Braces />} title={t('decode.header')}>
            <CodeEditor value={result?.header ?? ''} readOnly language="json" aria-label={t('decode.header')} />
          </Panel>
          <Panel icon={<Braces />} title={t('decode.payload')}>
            <CodeEditor value={result?.payload ?? ''} readOnly language="json" lineWrapping aria-label={t('decode.payload')} />
          </Panel>
        </div>
      )}
    </div>
  )
}
