import type { ReactNode } from 'react'
import { Badge, Button, Progress, cn, toast } from '@toolforge/ui'
import { CopyButton, ValueRow, host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Download } from 'lucide-react'
import type { Certificate } from './types'

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-1">
      <h3 className="px-3 text-xs font-semibold text-fg-muted">{title}</h3>
      <div>{children}</div>
    </section>
  )
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-3 px-3 py-1.5 text-[13px]">
      <span className="w-28 shrink-0 text-xs font-semibold text-fg-muted">{label}</span>
      <span className="min-w-0 flex-1 break-words text-fg" data-selectable>
        {children}
      </span>
    </div>
  )
}

/** 有效期进度：已使用的比例 */
function lifetime(certificate: Certificate) {
  const start = Date.parse(certificate.notBefore.replace(' UTC', 'Z').replace(' ', 'T'))
  const end = Date.parse(certificate.notAfter.replace(' UTC', 'Z').replace(' ', 'T'))
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return null
  return Math.min(100, Math.max(0, ((Date.now() - start) / (end - start)) * 100))
}

export function CertificateDetails({ certificate }: { certificate: Certificate }) {
  const { t, errorMessage } = usePlugin()
  const used = lifetime(certificate)
  const expiring = certificate.validity === 'valid' && certificate.daysLeft < 30
  const tone = certificate.validity !== 'valid' ? 'danger' : expiring ? 'warning' : 'success'

  const save = async () => {
    try {
      const name = (certificate.subject.commonName ?? 'certificate').replace(/[^\w.-]+/g, '_')
      const path = await host.dialog.saveFile(`${name}.pem`, [{ name: 'PEM', extensions: ['pem', 'crt'] }])
      if (!path) return
      await host.fs.writeText(path, certificate.pem)
      toast.success(t('common:saved'))
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  const key = [certificate.key.algorithm, certificate.key.curve, certificate.key.bits ? t('details.bits', { bits: certificate.key.bits }) : null]
    .filter(Boolean)
    .join(' · ')

  return (
    <div className="space-y-4 py-3">
      <div className="space-y-2 px-3">
        <div className="flex flex-wrap items-center gap-2">
          <h2 className="min-w-0 truncate text-lg font-semibold text-fg" data-selectable>
            {certificate.subject.commonName ?? certificate.subject.dn}
          </h2>
          <Badge variant={tone}>
            {certificate.validity === 'valid'
              ? expiring
                ? t('validity.expiring', { days: certificate.daysLeft })
                : t('validity.valid', { days: certificate.daysLeft })
              : t(`validity.${certificate.validity}`)}
          </Badge>
          {certificate.isCa && <Badge variant="info">{t('details.ca')}</Badge>}
          {certificate.selfSigned && <Badge>{t('details.selfSigned')}</Badge>}
        </div>
        {used !== null && (
          <div className="space-y-1">
            <Progress value={used} className={cn(tone === 'danger' && '[&>div]:bg-danger', tone === 'warning' && '[&>div]:bg-warning')} />
            <div className="flex justify-between text-[11px] text-fg-subtle tabular-nums">
              <span>{certificate.notBefore}</span>
              <span>{certificate.notAfter}</span>
            </div>
          </div>
        )}
      </div>

      <Section title={t('details.names')}>
        <Row label={t('details.subject')}>{certificate.subject.dn}</Row>
        <Row label={t('details.issuer')}>{certificate.issuer.dn}</Row>
        {certificate.altNames.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-3 py-1.5">
            {certificate.altNames.map((name) => (
              <Badge key={`${name.kind}-${name.value}`} variant={name.kind === 'dns' ? 'primary' : 'neutral'}>
                <span className="font-mono" data-selectable>
                  {name.value}
                </span>
              </Badge>
            ))}
          </div>
        )}
      </Section>

      <Section title={t('details.technical')}>
        <Row label={t('details.key')}>{key}</Row>
        <Row label={t('details.signature')}>{certificate.signatureAlgorithm}</Row>
        <Row label={t('details.version')}>{`v${certificate.version}`}</Row>
        <ValueRow label={t('details.serial')} value={certificate.serial} labelWidth="w-28" />
        {certificate.isCa && <Row label={t('details.pathLen')}>{certificate.pathLen ?? t('details.unlimited')}</Row>}
        {certificate.keyUsage.length > 0 && <Row label={t('details.keyUsage')}>{certificate.keyUsage.join(', ')}</Row>}
        {certificate.extendedKeyUsage.length > 0 && <Row label={t('details.extendedKeyUsage')}>{certificate.extendedKeyUsage.join(', ')}</Row>}
      </Section>

      <Section title={t('details.fingerprints')}>
        <ValueRow label="SHA-256" value={certificate.fingerprints.sha256} labelWidth="w-28" />
        <ValueRow label="SHA-1" value={certificate.fingerprints.sha1} labelWidth="w-28" />
        {certificate.subjectKeyId && <ValueRow label={t('details.subjectKeyId')} value={certificate.subjectKeyId} labelWidth="w-28" />}
        {certificate.authorityKeyId && <ValueRow label={t('details.authorityKeyId')} value={certificate.authorityKeyId} labelWidth="w-28" />}
      </Section>

      {(certificate.ocsp.length > 0 || certificate.caIssuers.length > 0 || certificate.crl.length > 0) && (
        <Section title={t('details.links')}>
          {certificate.ocsp.map((url) => (
            <ValueRow key={url} label="OCSP" value={url} labelWidth="w-28" />
          ))}
          {certificate.caIssuers.map((url) => (
            <ValueRow key={url} label={t('details.caIssuers')} value={url} labelWidth="w-28" />
          ))}
          {certificate.crl.map((url) => (
            <ValueRow key={url} label="CRL" value={url} labelWidth="w-28" />
          ))}
        </Section>
      )}

      <Section title="PEM">
        <div className="space-y-2 px-3">
          <pre className="max-h-40 overflow-auto rounded-control bg-surface-2 p-2.5 font-mono text-[11px] leading-snug text-fg-muted" data-selectable>
            {certificate.pem}
          </pre>
          <div className="flex gap-2">
            <CopyButton text={certificate.pem} label={t('common:copy')} variant="outline" />
            <Button size="sm" variant="outline" onClick={save}>
              <Download />
              {t('details.save')}
            </Button>
          </div>
        </div>
      </Section>
    </div>
  )
}
