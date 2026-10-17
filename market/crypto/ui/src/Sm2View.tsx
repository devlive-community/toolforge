import { useState, type ReactNode } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, Panel, SegmentedControl, Select, Spinner, cn, toast } from '@toolforge/ui'
import { CopyButton, host, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BadgeCheck, BadgeX, FileDown, FileInput, FileOutput, KeyRound, ListTree, Play, RotateCcw, ShieldAlert, Undo2 } from 'lucide-react'
import { EncodingSelect } from './EncodingSelect'
import type {
  BinaryEncoding,
  Encoding,
  LaunchKey,
  SignatureFormat,
  SignatureInput,
  Sm2ConvertOutput,
  Sm2Details,
  Sm2KeyInfo,
  Sm2KeyPair,
  Sm2Operation,
  Sm2SignOutput,
  Sm2VerifyOutput,
  UserIdEncoding,
} from './types'

const DEFAULT_USER_ID = '1234567812345678'

/** 用户标识的字节数；十六进制不合法时返回 null */
function userIdBytes(text: string, encoding: UserIdEncoding): number | null {
  if (encoding === 'utf8') return new TextEncoder().encode(text).length
  const clean = text.replace(/[\s:]/g, '').replace(/^0x/i, '')
  return /^([0-9a-f]{2})*$/i.test(clean) ? clean.length / 2 : null
}

function KeyEditor({ label, value, onChange, actions }: { label: string; value: string; onChange: (value: string) => void; actions?: (info: Sm2KeyInfo) => ReactNode }) {
  const { t, errorMessage } = usePlugin()
  const key = value.trim()
  const { result, error } = useDebouncedCall<Sm2KeyInfo>(key ? 'sm2_inspect' : null, key ? { key: value } : null, [value])
  return (
    <div className="flex min-h-0 flex-col gap-1.5">
      <div className="flex min-h-control-sm items-center gap-2">
        <p className="shrink-0 text-xs font-medium text-fg-muted">{label}</p>
        {key &&
          (error ? (
            <Badge variant="danger">{t('sm2.invalidKey')}</Badge>
          ) : (
            result && <Badge variant="success">{t(`sm2.keyInfo.${result.kind}`, { format: t(`sm2.formats.${result.format}`) })}</Badge>
          ))}
        <div className="ml-auto flex items-center gap-1">
          {key && !error && result && actions?.(result)}
          <CopyButton text={value} disabled={!value} />
        </div>
      </div>
      <div className={cn('min-h-0 flex-1 overflow-hidden rounded-control border', key && error ? 'border-danger' : 'border-border')}>
        <CodeEditor value={value} onChange={onChange} language="text" lineWrapping placeholder={t('sm2.keyPlaceholder')} aria-label={label} />
      </div>
      {key && error && <p className="text-xs text-danger">{errorMessage(error)}</p>}
    </div>
  )
}

function Row({ label, value, mono = true }: { label: string; value: ReactNode; mono?: boolean }) {
  return (
    <div className="grid grid-cols-[96px_minmax(0,1fr)] gap-3 border-b border-border px-3 py-2 last:border-b-0">
      <dt className="text-xs text-fg-muted">{label}</dt>
      <dd className={cn('text-xs break-all text-fg', mono && 'font-mono')}>{value}</dd>
    </div>
  )
}

function SignatureFormatSelect({ value, onChange, auto }: { value: SignatureInput; onChange: (value: SignatureInput) => void; auto?: boolean }) {
  const { t } = usePlugin()
  const options: { value: SignatureInput; label: string }[] = [
    { value: 'der', label: t('sm2.signatureFormats.der') },
    { value: 'raw', label: t('sm2.signatureFormats.raw') },
  ]
  return (
    <Select<SignatureInput>
      size="sm"
      value={value}
      onValueChange={onChange}
      className="w-32"
      aria-label={t('sm2.signatureFormat')}
      options={auto ? [{ value: 'auto', label: t('sm2.signatureFormats.auto') }, ...options] : options}
    />
  )
}

function BinarySelect({ value, onChange, label }: { value: BinaryEncoding; onChange: (value: BinaryEncoding) => void; label: string }) {
  return (
    <Select<BinaryEncoding>
      size="sm"
      value={value}
      onValueChange={onChange}
      className="w-24"
      aria-label={label}
      options={[
        { value: 'hex', label: 'Hex' },
        { value: 'base64', label: 'Base64' },
      ]}
    />
  )
}

/** 一次签名 / 验签的结果，连同当时的输入设置（用于判断结果是否过期与生成报告） */
type Outcome =
  | { op: 'sign'; inputs: string; out: Sm2SignOutput }
  | { op: 'verify'; inputs: string; out: Sm2VerifyOutput }
  | { op: 'verify'; inputs: string; error: string }
  | { op: 'convert'; inputs: string; out: Sm2ConvertOutput }

export function Sm2View({ launch }: { launch: LaunchKey | null }) {
  const { t, call, errorMessage } = usePlugin()
  const [privateKey, setPrivateKey] = useState('')
  const [publicKey, setPublicKey] = useState('')
  const [operation, setOperation] = useState<Sm2Operation>('sign')
  const [userId, setUserId] = useState(DEFAULT_USER_ID)
  const [userIdEncoding, setUserIdEncoding] = useState<UserIdEncoding>('utf8')
  const [message, setMessage] = useState('Hello, SM2!')
  const [messageEncoding, setMessageEncoding] = useState<Encoding>('utf8')
  const [signFormat, setSignFormat] = useState<SignatureFormat>('der')
  const [verifyFormat, setVerifyFormat] = useState<SignatureInput>('auto')
  const [signatureEncoding, setSignatureEncoding] = useState<BinaryEncoding>('hex')
  const [signature, setSignature] = useState('')
  const [convertTo, setConvertTo] = useState<SignatureFormat>('raw')
  const [convertEncoding, setConvertEncoding] = useState<BinaryEncoding>('hex')
  const [outcome, setOutcome] = useState<Outcome | null>(null)
  const [busy, setBusy] = useState(false)

  // 剪贴板中的 SM2 PEM 密钥
  const [appliedLaunch, setAppliedLaunch] = useState(0)
  if (launch && launch.seq !== appliedLaunch && launch.label?.startsWith('sm2')) {
    setAppliedLaunch(launch.seq)
    if (launch.label === 'sm2PrivateKey') setPrivateKey(launch.text)
    else setPublicKey(launch.text)
  }

  // 输入一变，旧结果就不再对应当前内容
  const inputs = JSON.stringify(
    operation === 'convert'
      ? [operation, signature, verifyFormat, signatureEncoding, convertTo, convertEncoding]
      : [operation, operation === 'sign' ? privateKey : publicKey || privateKey, userId, userIdEncoding, message, messageEncoding, signFormat, verifyFormat, signatureEncoding, operation === 'verify' ? signature : ''],
  )
  const current = outcome?.inputs === inputs ? outcome : null
  const idBytes = userIdBytes(userId, userIdEncoding)

  const generate = async () => {
    try {
      const pair = await call<Sm2KeyPair>('sm2_generate')
      setPrivateKey(pair.privateKey)
      setPublicKey(pair.publicKey)
    } catch (err) {
      toast.error(errorMessage(err))
    }
  }

  const exportPrivate = async () => {
    try {
      const pem = await call<string>('sm2_export', { key: privateKey })
      const path = await host.dialog.saveFile('sm2-private-key.pem', [{ name: 'PEM', extensions: ['pem'] }])
      if (!path) return
      await host.fs.writeText(path, pem)
      toast.success(t('sm2.exported'))
    } catch (err) {
      toast.error(errorMessage(err))
    }
  }

  const base = () => ({ userId, userIdEncoding, message, messageEncoding })

  const run = async () => {
    setBusy(true)
    try {
      if (operation === 'sign') {
        const out = await call<Sm2SignOutput>('sm2_sign', { ...base(), key: privateKey, format: signFormat, encoding: signatureEncoding })
        setOutcome({ op: 'sign', inputs, out })
        setSignature(out.signature)
      } else if (operation === 'verify') {
        try {
          const out = await call<Sm2VerifyOutput>('sm2_verify', { ...base(), key: publicKey || privateKey, signature, format: verifyFormat, encoding: signatureEncoding })
          setOutcome({ op: 'verify', inputs, out })
        } catch (err) {
          // 输入非法与「不匹配」分开呈现
          setOutcome({ op: 'verify', inputs, error: errorMessage(err) })
        }
      } else {
        const out = await call<Sm2ConvertOutput>('sm2_convert', { signature, from: verifyFormat, encoding: signatureEncoding, to: convertTo, outputEncoding: convertEncoding })
        setOutcome({ op: 'convert', inputs, out })
      }
    } catch (err) {
      toast.error(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  const changeOperation = (next: Sm2Operation) => {
    setOperation(next)
    // 签名结果带到验签与格式转换
    if (current?.op === 'sign') setVerifyFormat(current.out.format)
  }

  const details: Sm2Details | null = current && current.op !== 'convert' && 'out' in current ? current.out.details : null
  const rs = current && 'out' in current ? current.out : null
  const verified = current?.op === 'verify' ? ('out' in current ? (current.out.valid ? 'valid' : 'mismatch') : 'invalid') : null
  const shownFormat = !rs ? null : 'from' in rs ? rs.from : rs.format

  const report = () => {
    if (!current || current.op === 'convert' || !('out' in current)) return ''
    const { out } = current
    const { details } = out
    const result = current.op === 'sign' ? t('sm2.report.signed') : verified === 'valid' ? t('sm2.valid') : t('sm2.mismatch')
    const lines = [
      t('sm2.report.title'),
      `${t('sm2.report.algorithm')}: SM2-with-SM3 (GB/T 32918.2-2016)`,
      `${t('sm2.report.operation')}: ${t(`sm2.${current.op}`)}`,
      `${t('sm2.report.result')}: ${result}`,
      `${t('sm2.userId')}: ${userIdEncoding === 'utf8' ? userId : '-'} (${t(`sm2.userIdEncodings.${userIdEncoding}`)})`,
      `${t('sm2.report.userIdHex')}: ${details.userId}`,
      `ENTLA: ${details.entla}`,
      `${t('sm2.report.message')}: ${messageEncoding === 'utf8' ? t('encodings.utf8') : messageEncoding === 'hex' ? 'Hex' : 'Base64'}, ${t('sm2.bytes', { count: details.messageBytes })}`,
      `${t('sm2.report.messageHex')}: ${details.messagePreview}${details.messageBytes > 256 ? ' …' : ''}`,
      `${t('sm2.publicKey')}: ${details.publicKey}`,
      `ZA: ${details.za}`,
      `e = SM3(ZA ‖ M): ${details.e}`,
      `${t('sm2.signatureFormat')}: ${t(`sm2.signatureFormats.${out.format}`)}, ${signatureEncoding === 'hex' ? 'Hex' : 'Base64'}`,
      `r: ${out.r}`,
      `s: ${out.s}`,
    ]
    return lines.join('\n')
  }

  const canRun = operation === 'sign' ? !!privateKey.trim() : operation === 'verify' ? !!(publicKey || privateKey).trim() && !!signature.trim() : !!signature.trim()

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <div className="grid min-h-0 grid-rows-[minmax(0,3fr)_minmax(0,2fr)] gap-3">
        <Panel
          icon={<KeyRound />}
          title={t('sm2.keys')}
          actions={
            <Button size="sm" variant="primary" onClick={generate}>
              <KeyRound />
              {t('sm2.generate')}
            </Button>
          }
          bodyClassName="grid min-h-0 grid-rows-2 gap-3 p-3"
        >
          <KeyEditor
            label={t('sm2.privateKey')}
            value={privateKey}
            onChange={setPrivateKey}
            actions={(info) =>
              info.kind === 'private' && (
                <>
                  <Button size="sm" variant="ghost" onClick={() => setPublicKey(info.publicKeyPem)} disabled={publicKey === info.publicKeyPem}>
                    <Undo2 />
                    {t('sm2.derivePublic')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={exportPrivate}>
                    <FileDown />
                    {t('sm2.export')}
                  </Button>
                </>
              )
            }
          />
          <KeyEditor label={t('sm2.publicKey')} value={publicKey} onChange={setPublicKey} />
        </Panel>

        <Panel
          icon={<ListTree />}
          title={t('sm2.details')}
          actions={details && <CopyButton text={report()} label={t('sm2.copyReport')} variant="outline" />}
          bodyClassName="overflow-auto"
        >
          {rs ? (
            <dl>
              {details && (
                <>
                  <Row label={t('sm2.userId')} value={`${details.userId || '∅'}  (ENTLA ${details.entla})`} />
                  <Row label={t('sm2.report.message')} value={`${t('sm2.bytes', { count: details.messageBytes })} · ${details.messagePreview || '∅'}${details.messageBytes > 256 ? ' …' : ''}`} />
                  <Row label="ZA" value={details.za} />
                  <Row label="e = SM3(ZA ‖ M)" value={details.e} />
                  <Row label={t('sm2.publicKey')} value={details.publicKey} />
                </>
              )}
              {shownFormat && <Row label={t('sm2.signatureFormat')} value={t(`sm2.signatureFormats.${shownFormat}`)} mono={false} />}
              <Row label="r" value={rs.r} />
              <Row label="s" value={rs.s} />
            </dl>
          ) : (
            <Empty icon={<ListTree />} title={t('sm2.detailsEmpty')} description={t('sm2.detailsHint')} />
          )}
        </Panel>
      </div>

      <div className={cn('grid min-h-0 gap-3', operation === 'convert' ? 'grid-rows-[auto_minmax(0,1fr)_minmax(0,1fr)]' : 'grid-rows-[auto_auto_minmax(0,1fr)_minmax(0,1fr)]')}>
        <div className="flex flex-wrap items-center gap-3">
          <SegmentedControl<Sm2Operation>
            value={operation}
            onValueChange={changeOperation}
            aria-label={t('rsa.operation')}
            options={(['sign', 'verify', 'convert'] as Sm2Operation[]).map((o) => ({ value: o, label: t(`sm2.${o}`) }))}
          />
          <Button variant="primary" className="ml-auto" onClick={run} disabled={busy || !canRun}>
            {busy ? <Spinner /> : <Play />}
            {t(`sm2.${operation}`)}
          </Button>
        </div>

        {operation !== 'convert' && (
          <div className="flex flex-col gap-1">
            <div className="flex items-center gap-2">
              <Input
                size="sm"
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
                leading={<span className="text-xs font-medium whitespace-nowrap text-fg-muted">{t('sm2.userId')}</span>}
                invalid={idBytes === null}
                className="font-mono"
                wrapperClassName="flex-1"
                aria-label={t('sm2.userId')}
              />
              <Select<UserIdEncoding>
                size="sm"
                value={userIdEncoding}
                onValueChange={setUserIdEncoding}
                className="w-24"
                aria-label={t('sm2.userIdEncoding')}
                options={[
                  { value: 'utf8', label: t('sm2.userIdEncodings.utf8') },
                  { value: 'hex', label: t('sm2.userIdEncodings.hex') },
                ]}
              />
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setUserId(DEFAULT_USER_ID)
                  setUserIdEncoding('utf8')
                }}
                disabled={userId === DEFAULT_USER_ID && userIdEncoding === 'utf8'}
              >
                <RotateCcw />
                {t('sm2.resetUserId')}
              </Button>
            </div>
            <p className="text-xs text-fg-subtle">
              {idBytes === null ? t('sm2.userIdInvalidHex') : t('sm2.userIdHint', { count: idBytes, bits: idBytes * 8, value: DEFAULT_USER_ID })}
            </p>
          </div>
        )}

        {operation !== 'convert' && (
          <Panel
            icon={<FileInput />}
            title={t('sm2.message')}
            extra={<span className="truncate text-xs text-fg-subtle">{t('sm2.messageHint')}</span>}
            actions={<EncodingSelect value={messageEncoding} onChange={setMessageEncoding} label={t('rsa.inputEncoding')} />}
          >
            <CodeEditor value={message} onChange={setMessage} language="text" lineWrapping aria-label={t('sm2.message')} />
          </Panel>
        )}

        <Panel
          icon={verified === 'valid' ? <BadgeCheck /> : verified === 'mismatch' ? <BadgeX /> : verified === 'invalid' ? <ShieldAlert /> : operation === 'sign' ? <FileOutput /> : <FileInput />}
          title={operation === 'convert' ? t('sm2.convertInput') : t('sm2.signature')}
          extra={
            verified && (
              <Badge variant={verified === 'valid' ? 'success' : verified === 'mismatch' ? 'danger' : 'warning'}>{t(`sm2.${verified}`)}</Badge>
            )
          }
          actions={
            <>
              {operation === 'sign' ? (
                <SignatureFormatSelect value={signFormat} onChange={(v) => setSignFormat(v as SignatureFormat)} />
              ) : (
                <SignatureFormatSelect value={verifyFormat} onChange={setVerifyFormat} auto />
              )}
              <BinarySelect value={signatureEncoding} onChange={setSignatureEncoding} label={t('sm2.signatureEncoding')} />
              {operation === 'sign' && <CopyButton text={current?.op === 'sign' ? current.out.signature : ''} label={t('common:copy')} variant="outline" disabled={current?.op !== 'sign'} />}
            </>
          }
          bodyClassName={cn('flex min-h-0 flex-col', verified === 'valid' && 'bg-success-soft/40', verified === 'mismatch' && 'bg-danger-soft/40')}
        >
          <div className="min-h-0 flex-1">
            {operation === 'sign' ? (
              <CodeEditor value={current?.op === 'sign' ? current.out.signature : ''} readOnly language="text" lineWrapping aria-label={t('sm2.signature')} />
            ) : (
              <CodeEditor value={signature} onChange={setSignature} language="text" lineWrapping placeholder={t('sm2.signaturePlaceholder')} aria-label={t('sm2.signature')} />
            )}
          </div>
          {current?.op === 'verify' && 'error' in current && (
            <p className="shrink-0 border-t border-border bg-warning-soft/40 px-3 py-2 text-xs text-warning">{current.error}</p>
          )}
        </Panel>

        {operation === 'convert' && (
          <Panel
            icon={<FileOutput />}
            title={t('sm2.convertOutput')}
            actions={
              <>
                <SignatureFormatSelect value={convertTo} onChange={(v) => setConvertTo(v as SignatureFormat)} />
                <BinarySelect value={convertEncoding} onChange={setConvertEncoding} label={t('sm2.signatureEncoding')} />
                <CopyButton text={current?.op === 'convert' ? current.out.signature : ''} label={t('common:copy')} variant="outline" disabled={current?.op !== 'convert'} />
              </>
            }
          >
            <CodeEditor value={current?.op === 'convert' ? current.out.signature : ''} readOnly language="text" lineWrapping aria-label={t('sm2.convertOutput')} />
          </Panel>
        )}
      </div>
    </div>
  )
}
