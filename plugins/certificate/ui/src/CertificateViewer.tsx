import { useEffect, useState } from 'react'
import { Badge, Button, CodeEditor, Empty, Input, Panel, Spinner, Tabs, cn, toast } from '@toolforge/ui'
import { host, useDebouncedCall, useLaunchInput, usePlugin, useTask } from '@toolforge/plugin-ui-sdk'
import { FileKey, FolderOpen, Globe, Link2, Play, ShieldAlert, ShieldCheck, Square } from 'lucide-react'
import { CertificateDetails } from './CertificateDetails'
import type { Certificate, FetchReport, Parsed } from './types'

type Mode = 'paste' | 'server'
const EXTENSIONS = ['pem', 'crt', 'cer', 'der', 'cert']

function role(certificate: Certificate, index: number, total: number) {
  if (certificate.selfSigned && certificate.isCa) return 'root'
  if (index === 0 && !certificate.isCa) return 'leaf'
  return index === total - 1 && certificate.isCa ? 'root' : 'intermediate'
}

export function CertificateViewer() {
  const { t, call, errorMessage } = usePlugin()
  const [mode, setMode] = useState<Mode>('paste')
  const [text, setText] = useState('')
  const [fromFile, setFromFile] = useState<{ name: string; parsed: Parsed } | null>(null)
  const [hostName, setHostName] = useState('github.com')
  const [port, setPort] = useState('443')
  const [selected, setSelected] = useState(0)
  const task = useTask<FetchReport>()

  useLaunchInput((value) => {
    setMode('paste')
    setFromFile(null)
    setText(value)
    setSelected(0)
  })

  const pasted = useDebouncedCall<Parsed>(text.trim() ? 'parse' : null, text.trim() ? { text } : null, [text])
  const report = task.result
  const parsed: Parsed | null = mode === 'server' ? report : (fromFile?.parsed ?? pasted.result)
  const certificates = parsed?.certificates ?? []
  const current = certificates[Math.min(selected, certificates.length - 1)]

  useEffect(() => {
    if (task.status === 'failed' && task.error && task.error.code !== 'task.cancelled') toast.error(errorMessage(task.error))
  }, [task.status, task.error, errorMessage])

  const openFile = async () => {
    try {
      const path = await host.dialog.openFile([{ name: t('input.filter'), extensions: EXTENSIONS }])
      if (!path) return
      const result = await call<Parsed>('parse_file', { path })
      setFromFile({ name: path.split(/[\\/]/).pop() ?? path, parsed: result })
      setText('')
      setSelected(0)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const fetchChain = () => {
    setSelected(0)
    task.start('fetch', { host: hostName, port: Number(port) || 443 })
  }

  const verdictTone = report ? (report.verdict === 'trusted' ? 'success' : 'danger') : 'neutral'

  return (
    <div className="grid h-full min-h-0 grid-cols-[minmax(300px,380px)_minmax(0,1fr)] gap-3">
      <div className="flex min-h-0 flex-col gap-3">
        <Tabs<Mode>
          value={mode}
          onValueChange={(next) => {
            setMode(next)
            setSelected(0)
          }}
          aria-label={t('input.mode')}
          className="self-start"
          items={[
            { value: 'paste', label: t('input.paste'), icon: <FileKey /> },
            { value: 'server', label: t('input.server'), icon: <Globe /> },
          ]}
        />
        {mode === 'paste' ? (
          <Panel
            icon={<FileKey />}
            title={fromFile ? fromFile.name : t('input.pemTitle')}
            actions={
              <Button size="sm" onClick={openFile}>
                <FolderOpen />
                {t('input.open')}
              </Button>
            }
            footer={pasted.error && !fromFile && <span className="truncate text-danger">{errorMessage(pasted.error)}</span>}
            bodyClassName="p-0"
          >
            <CodeEditor
              value={text}
              onChange={(value) => {
                setText(value)
                setFromFile(null)
                setSelected(0)
              }}
              language="text"
              placeholder={t('input.placeholder')}
              aria-label={t('input.pemTitle')}
            />
          </Panel>
        ) : (
          <Panel icon={<Globe />} title={t('server.title')} className="shrink-0" bodyClassName="space-y-3 p-3">
            <div className="grid grid-cols-[1fr_88px] gap-2">
              <label className="flex flex-col gap-1.5">
                <span className="text-xs font-medium text-fg-muted">{t('server.host')}</span>
                <Input
                  value={hostName}
                  onChange={(event) => setHostName(event.target.value)}
                  onKeyDown={(event) => event.key === 'Enter' && !task.running && fetchChain()}
                  className="font-mono"
                  aria-label={t('server.host')}
                  spellCheck={false}
                />
              </label>
              <label className="flex flex-col gap-1.5">
                <span className="text-xs font-medium text-fg-muted">{t('server.port')}</span>
                <Input value={port} onChange={(event) => setPort(event.target.value.replace(/\D/g, '').slice(0, 5))} className="font-mono" aria-label={t('server.port')} />
              </label>
            </div>
            {task.running ? (
              <Button block onClick={task.cancel}>
                <Spinner />
                {t('server.fetching')}
                <Square className="ml-auto" />
              </Button>
            ) : (
              <Button block variant="primary" onClick={fetchChain} disabled={!hostName.trim()}>
                <Play />
                {t('server.fetch')}
              </Button>
            )}
            {report && (
              <div className={cn('space-y-2 rounded-control p-3', verdictTone === 'success' ? 'bg-success-soft' : 'bg-danger-soft')}>
                <p className={cn('flex items-center gap-1.5 text-[13px] font-semibold', verdictTone === 'success' ? 'text-success' : 'text-danger')}>
                  {report.verdict === 'trusted' ? <ShieldCheck className="size-4" /> : <ShieldAlert className="size-4" />}
                  {t(`verdicts.${report.verdict}`)}
                </p>
                {report.verdictDetail && <p className="text-xs break-words text-fg-muted">{report.verdictDetail}</p>}
                <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
                  <dt className="text-fg-muted">{t('server.address')}</dt>
                  <dd className="font-mono text-fg" data-selectable>
                    {`${report.address}:${report.port}`}
                    {report.fakeIp && <span className="ml-1.5 font-sans text-fg-muted">{t('server.viaProxy')}</span>}
                  </dd>
                  {report.protocol && (
                    <>
                      <dt className="text-fg-muted">{t('server.protocol')}</dt>
                      <dd className="text-fg">{report.protocol}</dd>
                    </>
                  )}
                  {report.cipher && (
                    <>
                      <dt className="text-fg-muted">{t('server.cipher')}</dt>
                      <dd className="font-mono break-all text-fg">{report.cipher}</dd>
                    </>
                  )}
                  <dt className="text-fg-muted">{t('server.elapsed')}</dt>
                  <dd className="text-fg tabular-nums">{t('server.ms', { ms: report.elapsedMs })}</dd>
                </dl>
              </div>
            )}
            <p className="text-[11px] text-fg-subtle">{t('server.hint')}</p>
          </Panel>
        )}
      </div>

      <Panel
        icon={<Link2 />}
        title={t('chain.title')}
        extra={certificates.length > 0 && <Badge>{t('chain.count', { count: certificates.length })}</Badge>}
        bodyClassName="flex min-h-0 flex-col"
      >
        {!current ? (
          <Empty icon={<FileKey />} title={t('chain.empty')} description={t('chain.emptyHint')} />
        ) : (
          <>
            <div className="flex shrink-0 gap-2 overflow-x-auto border-b border-border p-3">
              {certificates.map((certificate, index) => (
                <button
                  key={certificate.fingerprints.sha256}
                  type="button"
                  onClick={() => setSelected(index)}
                  className={cn(
                    'min-w-40 max-w-60 shrink-0 rounded-control border px-3 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
                    index === selected ? 'border-primary bg-primary-soft' : 'border-border hover:bg-hover',
                  )}
                >
                  <p className="text-[11px] text-fg-subtle">{t(`roles.${role(certificate, index, certificates.length)}`)}</p>
                  <p className="truncate text-[13px] font-medium text-fg">{certificate.subject.commonName ?? certificate.subject.dn}</p>
                  <p className={cn('text-[11px]', certificate.validity === 'valid' ? 'text-fg-muted' : 'text-danger')}>
                    {certificate.validity === 'valid' ? t('chain.until', { date: certificate.notAfter.slice(0, 10) }) : t(`validity.${certificate.validity}`)}
                  </p>
                </button>
              ))}
            </div>
            {parsed && !parsed.ordered && <p className="mx-3 mt-3 rounded-control bg-warning-soft px-2.5 py-2 text-xs text-warning">{t('chain.unordered')}</p>}
            <div className="min-h-0 flex-1 overflow-auto">
              <CertificateDetails key={current.fingerprints.sha256} certificate={current} />
            </div>
          </>
        )}
      </Panel>
    </div>
  )
}
