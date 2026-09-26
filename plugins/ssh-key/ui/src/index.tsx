import { useEffect, useState } from 'react'
import { Button, CodeEditor, Empty, Input, Panel, SegmentedControl, Select, Switch, Tabs, toast } from '@toolforge/ui'
import { CopyButton, host, useLaunchInput, useDebouncedCall, usePlugin, usePluginState, useTask } from '@toolforge/plugin-ui-sdk'
import { ClipboardPaste, Eye, EyeOff, FileKey, KeyRound, LockKeyhole, Save, ScanSearch, Wand2 } from 'lucide-react'
import { KeyCard } from './KeyCard'
import type { Generated, Inspection, Kind } from './types'

type Tab = 'generate' | 'inspect' | 'passphrase'
type Family = 'ed25519' | 'ecdsa' | 'rsa'

const SIZES: Record<Exclude<Family, 'ed25519'>, Kind[]> = { ecdsa: ['ecdsa256', 'ecdsa384', 'ecdsa521'], rsa: ['rsa2048', 'rsa3072', 'rsa4096'] }

function PasswordPair({ value, confirm, onChange, onConfirm }: { value: string; confirm: string; onChange: (v: string) => void; onConfirm: (v: string) => void }) {
  const { t } = usePlugin()
  const [visible, setVisible] = useState(false)
  const toggle = (
    <Button size="icon-sm" variant="ghost" aria-label={t(visible ? 'passphrase.hide' : 'passphrase.show')} onClick={() => setVisible(!visible)}>
      {visible ? <EyeOff /> : <Eye />}
    </Button>
  )
  return (
    <div className="grid grid-cols-2 gap-2">
      <Input type={visible ? 'text' : 'password'} value={value} onChange={(e) => onChange(e.target.value)} placeholder={t('passphrase.placeholder')} trailing={toggle} aria-label={t('passphrase.new')} autoComplete="new-password" />
      <Input type={visible ? 'text' : 'password'} value={confirm} onChange={(e) => onConfirm(e.target.value)} placeholder={t('passphrase.confirm')} invalid={confirm !== '' && confirm !== value} aria-label={t('passphrase.confirm')} autoComplete="new-password" />
    </div>
  )
}

function useSave() {
  const { t, call, errorMessage } = usePlugin()
  return async (privateKey: string, publicKey: string, name: string) => {
    try {
      const path = await host.dialog.saveFile(name)
      if (!path) return
      // 保存对话框已确认覆盖
      await call('save', { path, private: privateKey, public: publicKey, overwrite: true })
      toast.success(t('save.done', { path }))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
}

function Generate() {
  const { t, errorMessage } = usePlugin()
  const [family, setFamily] = usePluginState<Family>('family', 'ed25519')
  const [size, setSize] = usePluginState<Kind>('size', 'rsa4096')
  const [comment, setComment] = usePluginState('comment', '')
  const [passphrase, setPassphrase] = useState('')
  const [confirm, setConfirm] = useState('')
  const [reveal, setReveal] = useState(false)
  const task = useTask<Generated>()
  const save = useSave()
  const kind: Kind = family === 'ed25519' ? 'ed25519' : SIZES[family].includes(size) ? size : SIZES[family][family === 'rsa' ? 2 : 0]

  useEffect(() => {
    if (task.status === 'failed' && task.error) toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const result = task.result
  const mismatch = passphrase !== confirm
  const fileName = `id_${family === 'ed25519' ? 'ed25519' : family}`

  return (
    <div className="grid h-full min-h-0 grid-cols-[320px_minmax(0,1fr)] gap-3">
      <Panel icon={<Wand2 />} title={t('generate.title')} bodyClassName="space-y-4 overflow-auto p-3">
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('generate.type')}</p>
          <SegmentedControl<Family>
            size="sm"
            value={family}
            onValueChange={setFamily}
            aria-label={t('generate.type')}
            options={[
              { value: 'ed25519', label: 'Ed25519' },
              { value: 'ecdsa', label: 'ECDSA' },
              { value: 'rsa', label: 'RSA' },
            ]}
          />
          <p className="text-[11px] leading-snug text-fg-subtle">{t(`generate.hint.${family}`)}</p>
        </div>
        {family !== 'ed25519' && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-fg-muted">{t('generate.size')}</p>
            <Select<Kind> size="sm" value={kind} onValueChange={setSize} aria-label={t('generate.size')} options={SIZES[family].map((value) => ({ value, label: t(`sizes.${value}`) }))} />
          </div>
        )}
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('generate.comment')}</p>
          <Input value={comment} onChange={(e) => setComment(e.target.value)} placeholder="you@laptop" aria-label={t('generate.comment')} />
        </div>
        <div className="space-y-1.5">
          <p className="text-xs font-medium text-fg-muted">{t('passphrase.label')}</p>
          <PasswordPair value={passphrase} confirm={confirm} onChange={setPassphrase} onConfirm={setConfirm} />
          <p className="text-[11px] leading-snug text-fg-subtle">{t('passphrase.hint')}</p>
        </div>
        <Button block variant="primary" loading={task.running} disabled={mismatch} onClick={() => task.start('generate', { kind, comment, passphrase: passphrase || null })}>
          <KeyRound />
          {t('generate.action')}
        </Button>
      </Panel>

      <div className="min-h-0 space-y-3 overflow-auto">
        {!result ? (
          <Panel className="h-full">
            <Empty icon={<KeyRound />} title={task.running ? t('generate.running') : t('generate.empty')} description={task.running ? undefined : t('generate.emptyHint')} />
          </Panel>
        ) : (
          <>
            <KeyCard info={{ ...result.info, kind: 'private' }} showLine={false} />
            <Panel
              icon={<LockKeyhole />}
              title={t('generate.private')}
              actions={
                <>
                  <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                    <Switch size="sm" checked={reveal} onCheckedChange={setReveal} aria-label={t('generate.reveal')} />
                    {t('generate.reveal')}
                  </label>
                  <CopyButton text={result.private} label={t('generate.copyPrivate')} variant="outline" />
                  <Button size="sm" variant="primary" onClick={() => save(result.private, result.public, fileName)}>
                    <Save />
                    {t('save.action')}
                  </Button>
                </>
              }
              bodyClassName="p-3"
            >
              <pre className="font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap text-fg-muted" data-selectable={reveal}>
                {reveal ? result.private : result.private.replace(/^(?!-----).+$/gm, (line) => '•'.repeat(Math.min(line.length, 64)))}
              </pre>
              <p className="mt-2 text-[11px] text-warning">{t('generate.privateHint')}</p>
            </Panel>
          </>
        )}
      </div>
    </div>
  )
}

function Inspect({ launched }: { launched: string | null }) {
  const { t, errorMessage } = usePlugin()
  const [text, setText] = useState(launched ?? '')
  const [passphrase, setPassphrase] = useState('')
  const save = useSave()
  const trimmed = text.trim()
  const inspection = useDebouncedCall<Inspection>(trimmed ? 'inspect' : null, trimmed ? { text, passphrase: passphrase || null } : null, [text, passphrase])
  const result = inspection.result

  const paste = async () => {
    try {
      setText(await host.clipboard.readText())
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const openFile = async () => {
    try {
      const path = await host.dialog.openFile()
      if (path) setText(await host.fs.readText(path))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }
  const hasEncrypted = result?.keys.some((k) => k.encrypted) || result?.problems.some((p) => p.code === 'ssh.wrong_passphrase')

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)] gap-3">
      <Panel
        icon={<FileKey />}
        title={t('inspect.input')}
        actions={
          <>
            <Button size="sm" onClick={paste}>
              <ClipboardPaste />
              {t('inspect.paste')}
            </Button>
            <Button size="sm" onClick={openFile}>
              <FileKey />
              {t('inspect.open')}
            </Button>
          </>
        }
        footer={
          hasEncrypted && (
            <Input size="sm" type="password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} placeholder={t('inspect.passphrase')} wrapperClassName="w-full" aria-label={t('inspect.passphrase')} />
          )
        }
        bodyClassName="p-0"
      >
        <CodeEditor value={text} onChange={setText} language="text" lineWrapping placeholder={t('inspect.placeholder')} aria-label={t('inspect.input')} />
      </Panel>
      <div className="min-h-0 space-y-3 overflow-auto">
        {!result || (result.keys.length === 0 && result.problems.length === 0) ? (
          <Panel className="h-full">
            <Empty icon={<ScanSearch />} title={t('inspect.empty')} description={t('inspect.emptyHint')} />
          </Panel>
        ) : (
          <>
            {result.problems.map((problem) => (
              <p key={problem.line} className="rounded-control bg-danger-soft px-3 py-2 text-xs text-danger">
                {t('info.line', { line: problem.line })} · {errorMessage({ code: problem.code, params: {} })}
              </p>
            ))}
            {result.keys.map((info) => (
              <KeyCard key={`${info.line}-${info.sha256}`} info={info} onSave={(converted) => save(converted, info.public, 'id_rsa')} />
            ))}
          </>
        )}
      </div>
    </div>
  )
}

function Passphrase() {
  const { t, call, errorMessage } = usePlugin()
  const [privateKey, setPrivateKey] = useState('')
  const [old, setOld] = useState('')
  const [next, setNext] = useState('')
  const [confirm, setConfirm] = useState('')
  const [result, setResult] = useState<string | null>(null)
  const save = useSave()
  const change = async () => {
    try {
      setResult(await call<string>('passphrase', { private: privateKey, old: old || null, new: next || null }))
    } catch (error) {
      setResult(null)
      toast.error(errorMessage(error))
    }
  }
  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-3">
      <Panel icon={<LockKeyhole />} title={t('passphrase.title')} footer={<p className="text-[11px] text-fg-subtle">{t('passphrase.removeHint')}</p>} bodyClassName="flex min-h-0 flex-col gap-3 p-3">
        <div className="min-h-40 flex-1 overflow-hidden rounded-control border border-border">
          <CodeEditor value={privateKey} onChange={setPrivateKey} language="text" lineWrapping placeholder="-----BEGIN OPENSSH PRIVATE KEY-----" aria-label={t('passphrase.privateKey')} />
        </div>
        <Input type="password" value={old} onChange={(e) => setOld(e.target.value)} placeholder={t('passphrase.old')} aria-label={t('passphrase.old')} autoComplete="current-password" />
        <PasswordPair value={next} confirm={confirm} onChange={setNext} onConfirm={setConfirm} />
        <Button variant="primary" disabled={!privateKey.trim() || next !== confirm} onClick={change}>
          <LockKeyhole />
          {next ? t('passphrase.change') : t('passphrase.remove')}
        </Button>
      </Panel>
      <Panel
        title={t('passphrase.result')}
        actions={
          result && (
            <>
              <CopyButton text={result} variant="outline" />
              <Button size="sm" variant="primary" onClick={() => save(result, '', 'id_key')}>
                <Save />
                {t('save.action')}
              </Button>
            </>
          )
        }
        bodyClassName="overflow-auto p-3"
      >
        {result ? (
          <pre className="font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap text-fg" data-selectable>
            {result}
          </pre>
        ) : (
          <Empty icon={<LockKeyhole />} title={t('passphrase.empty')} />
        )}
      </Panel>
    </div>
  )
}

function SshKey() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('generate')
  const [launched, setLaunched] = useState<{ text: string; seq: number } | null>(null)
  useLaunchInput((text) => {
    setTab('inspect')
    setLaunched((prev) => ({ text, seq: (prev?.seq ?? 0) + 1 }))
  })
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs<Tab>
        value={tab}
        onValueChange={setTab}
        className="self-start"
        aria-label={t('name')}
        items={[
          { value: 'generate', label: t('tabs.generate'), icon: <Wand2 /> },
          { value: 'inspect', label: t('tabs.inspect'), icon: <ScanSearch /> },
          { value: 'passphrase', label: t('tabs.passphrase'), icon: <LockKeyhole /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        {/* 保持挂载，切换页签不会丢失刚生成的密钥 */}
        <div className="h-full" hidden={tab !== 'generate'}>
          <Generate />
        </div>
        <div className="h-full" hidden={tab !== 'inspect'}>
          <Inspect key={launched?.seq ?? 0} launched={launched?.text ?? null} />
        </div>
        <div className="h-full" hidden={tab !== 'passphrase'}>
          <Passphrase />
        </div>
      </div>
    </div>
  )
}

export default SshKey
