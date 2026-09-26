import { useState, type ReactNode } from 'react'
import { Button, CodeEditor, Input, NumberInput, Panel, SegmentedControl, Select, Spinner, Switch, Tooltip, toast } from '@toolforge/ui'
import { CopyButton, ValueRow, useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import { ArrowDownUp, Dices, FileInput, FileOutput, SlidersHorizontal, TriangleAlert } from 'lucide-react'
import { EncodingSelect } from './EncodingSelect'
import type { Algorithm, Direction, Encoding, KeyEncoding, Mode, Padding, SymmetricOutput } from './types'

function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <div className="space-y-1.5">
      <p className="text-xs font-medium text-fg-muted">{label}</p>
      {children}
      {hint && <p className="text-[11px] text-fg-subtle">{hint}</p>}
    </div>
  )
}

export function SymmetricView() {
  const { t, call, errorMessage } = usePlugin()
  const [algorithm, setAlgorithm] = useState<Algorithm>('aes')
  const [mode, setMode] = useState<Mode>('gcm')
  const [keyBits, setKeyBits] = useState(256)
  const [key, setKey] = useState('')
  const [keyEncoding, setKeyEncoding] = useState<KeyEncoding>('hex')
  const [iv, setIv] = useState('')
  const [embedIv, setEmbedIv] = useState(true)
  const [salt, setSalt] = useState('')
  const [iterations, setIterations] = useState(100000)
  const [aad, setAad] = useState('')
  const [padding, setPadding] = useState<Padding>('pkcs7')
  const [direction, setDirection] = useState<Direction>('encrypt')
  const [input, setInput] = useState('Hello, ToolForge! 你好')
  const [inputEncoding, setInputEncoding] = useState<Encoding>('utf8')
  const [outputEncoding, setOutputEncoding] = useState<Encoding>('base64')

  const aead = algorithm === 'chacha20' || mode === 'gcm'
  const blockMode = algorithm !== 'chacha20' && (mode === 'cbc' || mode === 'ecb')
  const keyBytes = algorithm === 'aes' ? keyBits / 8 : algorithm === 'sm4' ? 16 : 32
  const ready = key.length > 0 && input.length > 0
  const args = ready
    ? { algorithm, mode, direction, input, inputEncoding, outputEncoding, key, keyEncoding, keyBits, iv, embedIv, salt, iterations, aad, padding }
    : null
  const { result, error, pending } = useDebouncedCall<SymmetricOutput>(args ? 'symmetric' : null, args, [JSON.stringify(args)])

  const randomKey = async () => {
    try {
      const encoding = keyEncoding === 'base64' ? 'base64' : 'hex'
      setKey(await call<string>('random', { length: keyBytes, encoding }))
      if (keyEncoding !== 'base64') setKeyEncoding('hex')
    } catch (err) {
      toast.error(errorMessage(err))
    }
  }

  /** 把输出作为新的输入并切换方向；随机生成的 IV 与盐一并填入，便于立即解密验证 */
  const swap = () => {
    if (!result) return
    setInput(result.output)
    setInputEncoding(outputEncoding)
    setOutputEncoding(inputEncoding)
    if (direction === 'encrypt') {
      if (!embedIv && result.iv) setIv(result.iv)
      if (result.salt) setSalt(result.salt)
    }
    setDirection(direction === 'encrypt' ? 'decrypt' : 'encrypt')
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,340px)_minmax(0,1fr)] gap-3">
      <Panel icon={<SlidersHorizontal />} title={t('symmetric.options')} bodyClassName="space-y-4 overflow-auto p-3">
        <Field label={t('symmetric.algorithm')}>
          <SegmentedControl<Algorithm>
            size="sm"
            value={algorithm}
            onValueChange={setAlgorithm}
            aria-label={t('symmetric.algorithm')}
            options={[
              { value: 'aes', label: 'AES' },
              { value: 'sm4', label: 'SM4' },
              { value: 'chacha20', label: 'ChaCha20' },
            ]}
          />
        </Field>
        {algorithm !== 'chacha20' && (
          <Field label={t('symmetric.mode')} hint={mode === 'ecb' ? t('symmetric.ecbWarning') : undefined}>
            <SegmentedControl<Mode>
              size="sm"
              value={mode}
              onValueChange={setMode}
              aria-label={t('symmetric.mode')}
              options={(['gcm', 'cbc', 'ctr', 'ecb'] as Mode[]).map((m) => ({ value: m, label: m.toUpperCase() }))}
            />
          </Field>
        )}
        {algorithm === 'aes' && (
          <Field label={t('symmetric.keyBits')}>
            <SegmentedControl<string>
              size="sm"
              value={String(keyBits)}
              onValueChange={(v) => setKeyBits(Number(v))}
              aria-label={t('symmetric.keyBits')}
              options={['128', '192', '256'].map((b) => ({ value: b, label: t('symmetric.bits', { bits: b }) }))}
            />
          </Field>
        )}
        <Field label={t('symmetric.key')} hint={keyEncoding === 'password' ? t('symmetric.passwordHint') : t('symmetric.keyHint', { bytes: keyBytes })}>
          <div className="flex gap-1.5">
            <Select<KeyEncoding>
              size="sm"
              value={keyEncoding}
              onValueChange={setKeyEncoding}
              className="w-28 shrink-0"
              aria-label={t('symmetric.keyEncoding')}
              options={[
                { value: 'hex', label: 'Hex' },
                { value: 'base64', label: 'Base64' },
                { value: 'utf8', label: t('encodings.utf8') },
                { value: 'password', label: t('symmetric.password') },
              ]}
            />
            {keyEncoding !== 'password' && keyEncoding !== 'utf8' && (
              <Tooltip content={t('symmetric.randomKey')}>
                <Button size="icon-sm" aria-label={t('symmetric.randomKey')} onClick={randomKey}>
                  <Dices />
                </Button>
              </Tooltip>
            )}
          </div>
          <Input size="sm" value={key} onChange={(event) => setKey(event.target.value)} className="font-mono" aria-label={t('symmetric.key')} spellCheck={false} />
        </Field>
        {keyEncoding === 'password' && (
          <div className="grid grid-cols-[1fr_auto] gap-2">
            <Field label={t('symmetric.salt')}>
              <Input size="sm" value={salt} onChange={(event) => setSalt(event.target.value)} placeholder={t('symmetric.randomWhenEmpty')} className="font-mono" aria-label={t('symmetric.salt')} />
            </Field>
            <Field label={t('symmetric.iterations')}>
              <NumberInput size="sm" value={iterations} onValueChange={setIterations} min={1} max={10_000_000} step={10000} className="w-32" aria-label={t('symmetric.iterations')} />
            </Field>
          </div>
        )}
        {(algorithm === 'chacha20' || mode !== 'ecb') && (
          <Field label={aead ? t('symmetric.nonce') : t('symmetric.iv')}>
            <Input size="sm" value={iv} onChange={(event) => setIv(event.target.value)} placeholder={t('symmetric.randomWhenEmpty')} className="font-mono" aria-label={t('symmetric.iv')} spellCheck={false} />
            <label className="flex items-center gap-2 pt-1 text-xs text-fg-muted">
              <Switch size="sm" checked={embedIv} onCheckedChange={setEmbedIv} aria-label={t('symmetric.embedIv')} />
              {t('symmetric.embedIv')}
            </label>
          </Field>
        )}
        {aead && (
          <Field label={t('symmetric.aad')}>
            <Input size="sm" value={aad} onChange={(event) => setAad(event.target.value)} placeholder={t('symmetric.optional')} aria-label={t('symmetric.aad')} />
          </Field>
        )}
        {blockMode && (
          <Field label={t('symmetric.padding')}>
            <Select<Padding>
              size="sm"
              value={padding}
              onValueChange={setPadding}
              aria-label={t('symmetric.padding')}
              options={[
                { value: 'pkcs7', label: 'PKCS#7' },
                { value: 'zero', label: t('symmetric.zeroPadding') },
                { value: 'none', label: t('symmetric.noPadding') },
              ]}
            />
          </Field>
        )}
      </Panel>

      <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)_minmax(0,1fr)] gap-3">
        <div className="flex items-center gap-2">
          <SegmentedControl<Direction>
            value={direction}
            onValueChange={(next) => {
              setDirection(next)
              setInputEncoding(next === 'encrypt' ? 'utf8' : 'base64')
              setOutputEncoding(next === 'encrypt' ? 'base64' : 'utf8')
            }}
            aria-label={t('symmetric.direction')}
            options={[
              { value: 'encrypt', label: t('symmetric.encrypt') },
              { value: 'decrypt', label: t('symmetric.decrypt') },
            ]}
          />
          <Button onClick={swap} disabled={!result || !!error}>
            <ArrowDownUp />
            {t('symmetric.swap')}
          </Button>
        </div>
        <Panel
          icon={<FileInput />}
          title={direction === 'encrypt' ? t('symmetric.plaintext') : t('symmetric.ciphertext')}
          actions={<EncodingSelect value={inputEncoding} onChange={setInputEncoding} label={t('symmetric.inputEncoding')} />}
        >
          <CodeEditor value={input} onChange={setInput} language="text" lineWrapping aria-label={t('symmetric.plaintext')} />
        </Panel>
        <Panel
          icon={<FileOutput />}
          title={direction === 'encrypt' ? t('symmetric.ciphertext') : t('symmetric.plaintext')}
          extra={pending && <Spinner className="size-3.5 text-fg-subtle" />}
          actions={
            <>
              <EncodingSelect value={outputEncoding} onChange={setOutputEncoding} label={t('symmetric.outputEncoding')} />
              <CopyButton text={result?.output ?? ''} label={t('common:copy')} variant="outline" disabled={!result || !!error} />
            </>
          }
          footer={error ? <span className="flex items-center gap-1.5 truncate text-danger"><TriangleAlert className="size-3.5 shrink-0" />{errorMessage(error)}</span> : !ready ? <span>{t('symmetric.needKey')}</span> : null}
          bodyClassName="flex min-h-0 flex-col"
        >
          <div className="min-h-0 flex-1">
            <CodeEditor value={error || !ready ? '' : (result?.output ?? '')} readOnly language="text" lineWrapping aria-label={t('symmetric.ciphertext')} />
          </div>
          {result && !error && (result.iv || result.salt) && (
            <div className="shrink-0 border-t border-border py-1">
              {result.iv && <ValueRow label={algorithm === 'chacha20' || mode === 'gcm' ? t('symmetric.nonce') : t('symmetric.iv')} value={result.iv} labelWidth="w-24" />}
              {result.salt && <ValueRow label={t('symmetric.salt')} value={result.salt} labelWidth="w-24" />}
              {result.derivedKey && <ValueRow label={t('symmetric.derivedKey')} value={result.derivedKey} labelWidth="w-24" />}
            </div>
          )}
        </Panel>
      </div>
    </div>
  )
}
