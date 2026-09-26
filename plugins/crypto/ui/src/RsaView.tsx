import { useEffect, useState, type ReactNode } from 'react'
import { Badge, Button, CodeEditor, Panel, SegmentedControl, Select, Spinner, cn, toast } from '@toolforge/ui'
import { CopyButton, useDebouncedCall, useLaunchInput, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { BadgeCheck, BadgeX, FileInput, FileOutput, KeyRound, Play } from 'lucide-react'
import { EncodingSelect } from './EncodingSelect'
import type { Encoding, Hash, KeyInfo, KeyPair, Operation, RsaPadding, Scheme } from './types'

function KeyEditor({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  const { t } = usePlugin()
  const { result, error } = useDebouncedCall<KeyInfo>(value.trim() ? 'rsa_inspect' : null, value.trim() ? { key: value } : null, [value])
  return (
    <div className="flex min-h-0 flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <p className="text-xs font-medium text-fg-muted">{label}</p>
        {value.trim() &&
          (error ? (
            <Badge variant="danger">{t('rsa.invalidKey')}</Badge>
          ) : (
            result && <Badge variant="success">{t(`rsa.${result.kind}Info`, { bits: result.bits })}</Badge>
          ))}
        <CopyButton text={value} className="ml-auto" disabled={!value} />
      </div>
      <div className="min-h-0 flex-1 overflow-hidden rounded-control border border-border">
        <CodeEditor value={value} onChange={onChange} language="text" placeholder="-----BEGIN …-----" aria-label={label} />
      </div>
    </div>
  )
}

function Option({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex items-center gap-1.5 text-xs text-fg-muted">
      {label}
      {children}
    </label>
  )
}

export function RsaView({ onActivate }: { onActivate: () => void }) {
  const { t, call, errorMessage } = usePlugin()
  const task = useTask<KeyPair>()
  const [bits, setBits] = useState(2048)
  const [format, setFormat] = useState<'pkcs8' | 'pkcs1'>('pkcs8')
  const [privateKey, setPrivateKey] = useState('')
  const [publicKey, setPublicKey] = useState('')
  const [operation, setOperation] = useState<Operation>('encrypt')
  const [padding, setPadding] = useState<RsaPadding>('oaep-sha256')
  const [scheme, setScheme] = useState<Scheme>('pkcs1v15')
  const [hash, setHash] = useState<Hash>('sha256')
  const [input, setInput] = useState('Hello, RSA!')
  const [inputEncoding, setInputEncoding] = useState<Encoding>('utf8')
  const [outputEncoding, setOutputEncoding] = useState<Encoding>('base64')
  const [signature, setSignature] = useState('')
  const [output, setOutput] = useState<{ text: string; valid?: boolean } | null>(null)
  const [busy, setBusy] = useState(false)

  // 剪贴板中的 PEM 密钥：切到 RSA 页并填入
  useLaunchInput((text, label) => {
    onActivate()
    if (label === 'privateKey') setPrivateKey(text)
    else setPublicKey(text)
  })

  const generated = task.status === 'succeeded' ? task.result : null
  const [appliedTask, setAppliedTask] = useState<string | null>(null)
  if (generated && task.taskId !== appliedTask) {
    setAppliedTask(task.taskId)
    setPrivateKey(generated.privateKey)
    setPublicKey(generated.publicKey)
  }

  useEffect(() => {
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const run = async () => {
    setBusy(true)
    setOutput(null)
    try {
      if (operation === 'encrypt' || operation === 'decrypt') {
        const key = operation === 'encrypt' ? publicKey || privateKey : privateKey
        const text = await call<string>(`rsa_${operation}`, { key, input, inputEncoding, outputEncoding, padding })
        setOutput({ text })
      } else if (operation === 'sign') {
        const text = await call<string>('rsa_sign', { key: privateKey, message: input, messageEncoding: inputEncoding, scheme, hash, signatureEncoding: outputEncoding })
        setOutput({ text })
        setSignature(text)
      } else {
        const result = await call<{ valid: boolean }>('rsa_verify', {
          key: publicKey || privateKey,
          message: input,
          messageEncoding: inputEncoding,
          scheme,
          hash,
          signatureEncoding: outputEncoding,
          signature,
        })
        setOutput({ text: '', valid: result.valid })
      }
    } catch (err) {
      toast.error(errorMessage(err))
    } finally {
      setBusy(false)
    }
  }

  const changeOperation = (next: Operation) => {
    setOperation(next)
    setOutput(null)
    // 解密的输入通常是 Base64 密文
    setInputEncoding(next === 'decrypt' ? 'base64' : 'utf8')
    setOutputEncoding(next === 'decrypt' ? 'utf8' : 'base64')
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(320px,0.9fr)_minmax(0,1.1fr)] gap-3">
      <Panel
        icon={<KeyRound />}
        title={t('rsa.keys')}
        actions={
          <>
            <Select<number>
              size="sm"
              value={bits}
              onValueChange={setBits}
              className="w-28"
              aria-label={t('rsa.bits')}
              options={[2048, 3072, 4096].map((b) => ({ value: b, label: t('rsa.bitsOption', { bits: b }) }))}
            />
            <Select<'pkcs8' | 'pkcs1'>
              size="sm"
              value={format}
              onValueChange={setFormat}
              className="w-28"
              aria-label={t('rsa.format')}
              options={[
                { value: 'pkcs8', label: 'PKCS#8' },
                { value: 'pkcs1', label: 'PKCS#1' },
              ]}
            />
            <Button size="sm" variant="primary" disabled={task.running} onClick={() => task.start('rsa_generate', { bits, format })}>
              {task.running ? <Spinner /> : <KeyRound />}
              {t('rsa.generate')}
            </Button>
          </>
        }
        bodyClassName="grid min-h-0 grid-rows-2 gap-3 p-3"
      >
        <KeyEditor label={t('rsa.privateKey')} value={privateKey} onChange={setPrivateKey} />
        <KeyEditor label={t('rsa.publicKey')} value={publicKey} onChange={setPublicKey} />
      </Panel>

      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)_minmax(0,1fr)] gap-3">
        <div className="flex flex-wrap items-center gap-3">
          <SegmentedControl<Operation>
            value={operation}
            onValueChange={changeOperation}
            aria-label={t('rsa.operation')}
            options={(['encrypt', 'decrypt', 'sign', 'verify'] as Operation[]).map((o) => ({ value: o, label: t(`rsa.${o}`) }))}
          />
          {operation === 'encrypt' || operation === 'decrypt' ? (
            <Option label={t('rsa.padding')}>
              <Select<RsaPadding>
                size="sm"
                value={padding}
                onValueChange={setPadding}
                className="w-40"
                aria-label={t('rsa.padding')}
                options={[
                  { value: 'oaep-sha256', label: 'OAEP (SHA-256)' },
                  { value: 'oaep-sha1', label: 'OAEP (SHA-1)' },
                  { value: 'pkcs1v15', label: 'PKCS#1 v1.5' },
                ]}
              />
            </Option>
          ) : (
            <>
              <Option label={t('rsa.scheme')}>
                <Select<Scheme>
                  size="sm"
                  value={scheme}
                  onValueChange={setScheme}
                  className="w-32"
                  aria-label={t('rsa.scheme')}
                  options={[
                    { value: 'pkcs1v15', label: 'PKCS#1 v1.5' },
                    { value: 'pss', label: 'PSS' },
                  ]}
                />
              </Option>
              <Option label={t('rsa.hash')}>
                <Select<Hash>
                  size="sm"
                  value={hash}
                  onValueChange={setHash}
                  className="w-28"
                  aria-label={t('rsa.hash')}
                  options={(['sha256', 'sha384', 'sha512'] as Hash[]).map((h) => ({ value: h, label: h.toUpperCase().replace('SHA', 'SHA-') }))}
                />
              </Option>
            </>
          )}
          <Button variant="primary" className="ml-auto" onClick={run} disabled={busy || !input || (operation === 'verify' && !signature)}>
            {busy ? <Spinner /> : <Play />}
            {t(`rsa.${operation}`)}
          </Button>
        </div>

        <Panel
          icon={<FileInput />}
          title={operation === 'decrypt' ? t('rsa.ciphertext') : operation === 'encrypt' ? t('rsa.plaintext') : t('rsa.message')}
          actions={<EncodingSelect value={inputEncoding} onChange={setInputEncoding} label={t('rsa.inputEncoding')} />}
        >
          <CodeEditor value={input} onChange={setInput} language="text" lineWrapping aria-label={t('rsa.message')} />
        </Panel>

        <Panel
          icon={operation === 'verify' ? (output?.valid ? <BadgeCheck /> : <BadgeX />) : <FileOutput />}
          title={operation === 'verify' ? t('rsa.signature') : operation === 'sign' ? t('rsa.signature') : operation === 'encrypt' ? t('rsa.ciphertext') : t('rsa.plaintext')}
          extra={
            operation === 'verify' &&
            output?.valid !== undefined && (
              <Badge variant={output.valid ? 'success' : 'danger'}>{output.valid ? t('rsa.valid') : t('rsa.invalid')}</Badge>
            )
          }
          actions={
            <>
              <EncodingSelect value={outputEncoding} onChange={setOutputEncoding} label={t('rsa.outputEncoding')} />
              {operation !== 'verify' && <CopyButton text={output?.text ?? ''} label={t('common:copy')} variant="outline" disabled={!output?.text} />}
            </>
          }
          bodyClassName={cn(operation === 'verify' && output?.valid !== undefined && (output.valid ? 'bg-success-soft/40' : 'bg-danger-soft/40'))}
        >
          {operation === 'verify' ? (
            <CodeEditor value={signature} onChange={setSignature} language="text" lineWrapping placeholder={t('rsa.signaturePlaceholder')} aria-label={t('rsa.signature')} />
          ) : (
            <CodeEditor value={output?.text ?? ''} readOnly language="text" lineWrapping aria-label={t('rsa.output')} />
          )}
        </Panel>
      </div>
    </div>
  )
}
