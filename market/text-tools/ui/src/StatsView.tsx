import { Spinner } from '@toolforge/ui'
import { useDebouncedCall, usePlugin } from '@toolforge/plugin-ui-sdk'
import type { Stats } from './types'

const CARDS: (keyof Omit<Stats, 'topWords' | 'readingSeconds'>)[] = [
  'characters',
  'charactersNoSpaces',
  'words',
  'cjkCharacters',
  'latinWords',
  'lines',
  'nonEmptyLines',
  'paragraphs',
  'sentences',
  'bytes',
]

export function StatsView({ input }: { input: string }) {
  const { t } = usePlugin()
  const { result } = useDebouncedCall<Stats>('stats', { input }, [input])
  if (!result) return <Spinner className="m-6 text-fg-subtle" />
  const number = new Intl.NumberFormat()
  const minutes = Math.ceil(result.readingSeconds / 60)
  const max = result.topWords[0]?.[1] ?? 1

  return (
    <div className="space-y-5 p-4">
      <div className="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-2">
        {CARDS.map((key) => (
          <div key={key} className="rounded-card border border-border bg-surface-2 px-3 py-2.5">
            <p className="text-xs text-fg-muted">{t(`stats.${key}`)}</p>
            <p className="mt-1 font-mono text-xl font-semibold text-fg tabular-nums" data-selectable>
              {number.format(result[key])}
            </p>
          </div>
        ))}
        <div className="rounded-card border border-primary/30 bg-primary-soft/50 px-3 py-2.5">
          <p className="text-xs text-primary-fg">{t('stats.reading')}</p>
          <p className="mt-1 text-xl font-semibold text-fg">
            {result.readingSeconds === 0 ? '—' : result.readingSeconds < 60 ? t('stats.underMinute') : t('stats.minutes', { count: minutes })}
          </p>
        </div>
      </div>
      {result.topWords.length > 0 && (
        <section>
          <h3 className="mb-2 text-xs font-semibold tracking-wide text-fg-muted uppercase">{t('stats.topWords')}</h3>
          <ul className="space-y-1.5">
            {result.topWords.map(([word, count]) => (
              <li key={word} className="flex items-center gap-3 text-[13px]">
                <span className="w-32 truncate font-mono text-fg" data-selectable>
                  {word}
                </span>
                <div className="h-2 flex-1 overflow-hidden rounded-full bg-active">
                  <div className="h-full rounded-full bg-primary" style={{ width: `${(count / max) * 100}%` }} />
                </div>
                <span className="w-8 text-right text-xs text-fg-muted tabular-nums">{count}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  )
}
