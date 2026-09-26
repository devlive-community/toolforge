import { Badge, Button, Tooltip } from '@toolforge/ui'
import { CopyButton, usePlugin } from '@toolforge/plugin-ui-sdk'
import { KeyRound, Lock, Save, Server, ShieldCheck } from 'lucide-react'
import type { Info } from './types'

function Row({ label, value, mono = true }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid grid-cols-[88px_minmax(0,1fr)_auto] items-start gap-2 py-1">
      <span className="text-xs text-fg-muted">{label}</span>
      <span className={mono ? 'font-mono text-[12px] break-all text-fg' : 'text-[13px] text-fg'} data-selectable>
        {value}
      </span>
      <CopyButton text={value} />
    </div>
  )
}

/** 一把密钥的信息：类型、指纹、随机图与公钥 */
export function KeyCard({ info, onSave, showLine = true }: { info: Info; onSave?: (privateKey: string) => void; showLine?: boolean }) {
  const { t } = usePlugin()
  return (
    <section className="rounded-card border border-border bg-surface p-3">
      <header className="mb-2 flex flex-wrap items-center gap-2">
        {info.kind === 'private' ? <KeyRound className="size-4 text-warning" /> : <ShieldCheck className="size-4 text-primary" />}
        <span className="text-[13px] font-medium text-fg">{`${info.label} ${info.bits}`}</span>
        <Badge>{t(`kinds.${info.kind}`)}</Badge>
        {info.encrypted && (
          <Tooltip content={t('info.encryptedHint', { cipher: info.cipher ?? '' })}>
            <Badge variant="info">
              <Lock className="size-3" />
              {t('info.encrypted')}
            </Badge>
          </Tooltip>
        )}
        {info.comment && <span className="truncate text-xs text-fg-muted">{info.comment}</span>}
        {showLine && <span className="ml-auto text-[11px] text-fg-subtle">{t('info.line', { line: info.line })}</span>}
      </header>
      <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto]">
        <div className="min-w-0">
          <Row label={t('info.algorithm')} value={info.algorithm} />
          <Row label="SHA256" value={info.sha256} />
          <Row label="MD5" value={info.md5} />
          {info.hosts && <Row label={t('info.hosts')} value={info.hosts} />}
          {info.options && <Row label={t('info.options')} value={info.options} />}
          <Row label={t('info.public')} value={info.public} />
          {info.converted && (
            <div className="mt-2 flex flex-wrap items-center gap-2 rounded-control bg-info-soft px-2.5 py-2 text-xs text-info">
              <Server className="size-3.5" />
              {t('info.converted')}
              <span className="ml-auto flex gap-1.5">
                <CopyButton text={info.converted} label={t('info.copyConverted')} variant="outline" />
                {onSave && (
                  <Button size="sm" onClick={() => onSave(info.converted!)}>
                    <Save />
                    {t('save.action')}
                  </Button>
                )}
              </span>
            </div>
          )}
        </div>
        <pre className="h-fit rounded-control bg-surface-2 px-2.5 py-2 font-mono text-[11px] leading-[1.25] text-fg-muted" aria-label={t('info.randomart')}>
          {info.randomart}
        </pre>
      </div>
    </section>
  )
}
