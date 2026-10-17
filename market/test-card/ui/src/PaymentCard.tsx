import { CopyButton, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Wifi } from 'lucide-react'
import { groupNumber, type Bank, type Brand, type Card } from './types'
import './payment-card.css'

const ACCENTS: Record<Brand, string> = {
  visa: 'blue',
  mastercard: 'orange',
  amex: 'cyan',
  discover: 'orange',
  jcb: 'green',
  unionpay: 'pink',
  diners: 'violet',
}

const BANK_TONES: Record<Bank, string> = {
  cmb: 'red', ccb: 'blue', icbc: 'red', abc: 'green', boc: 'red', bocom: 'blue',
  psbc: 'green', cib: 'blue', spdb: 'blue', citic: 'red', cmbc: 'green', ceb: 'purple',
  pab: 'orange', cgb: 'red', hxb: 'red', bob: 'red', bosh: 'blue', bon: 'orange',
}

function Chip() {
  return (
    <span className="tf-test-card__chip" aria-hidden="true">
      <svg viewBox="0 0 48 36" fill="none" stroke="currentColor" strokeWidth="0.8">
        <rect x="15" y="6" width="18" height="24" rx="5" />
        <path d="M0 12h15M0 24h15M33 12h15M33 24h15M24 0v6M24 30v6M8 0v7l7 5M40 0v7l-7 5M8 36v-7l7-5M40 36v-7l-7-5" />
      </svg>
    </span>
  )
}

/** A presentation of local test data; every value comes from the Rust result. */
export function PaymentCard({ card }: { card: Card }) {
  const { t } = usePlugin()
  const bankName = card.bank ? t(`banks.${card.bank}`) : null
  return (
    <article className="tf-test-card" data-accent={ACCENTS[card.brand]} data-bank-tone={card.bank ? BANK_TONES[card.bank] : undefined} aria-label={t('card.label', { brand: bankName ?? t(`brands.${card.brand}`) })}>
      <div className="tf-test-card__content">
        <div className="tf-test-card__header">
          <span className="tf-test-card__brand" title={bankName ?? undefined}>{bankName ?? t(`brands.${card.brand}`)}</span>
          <div className="tf-test-card__actions">
            <span className="tf-test-card__test">{t('generate.testOnly')}</span>
            <CopyButton text={card.number} aria-label={t('card.copyNumber')} className="tf-test-card__copy" />
          </div>
        </div>

        <div className="tf-test-card__technology">
          <span className="tf-test-card__chip-group" aria-hidden="true"><Chip /><Wifi className="size-5 rotate-90" strokeWidth={1.5} /></span>
          {card.bank && <span className="tf-test-card__network">{t(`brands.${card.brand}`)}</span>}
        </div>

        <code className="tf-test-card__number" aria-label={t('fields.number')} data-selectable>
          {groupNumber(card.number, card.brand)}
        </code>

        <dl className="tf-test-card__details">
          <div className="min-w-0">
            <dt>{t('fields.holder')}</dt>
            <dd className="truncate" title={card.holder} data-selectable>{card.holder}</dd>
          </div>
          <div>
            <dt>{t('fields.expiry')}</dt>
            <dd data-selectable>{card.expiry}</dd>
          </div>
          <div>
            <dt>{t('fields.cvc')}</dt>
            <dd data-selectable>{card.cvc}</dd>
          </div>
        </dl>
      </div>
    </article>
  )
}
