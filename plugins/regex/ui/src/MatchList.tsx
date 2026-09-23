import { Empty, cn } from '@toolforge/ui'
import { CopyButton, usePlugin } from '@toolforge/plugin-ui-sdk'
import { SearchX } from 'lucide-react'
import type { Match } from './types'

interface Props {
  matches: Match[]
  active: number | null
  onSelect: (index: number) => void
}

export function MatchList({ matches, active, onSelect }: Props) {
  const { t } = usePlugin()
  if (matches.length === 0) return <Empty icon={<SearchX />} title={t('matches.none')} />
  return (
    <ul className="space-y-2 p-2">
      {matches.map((match, index) => (
        <li key={match.index}>
          <div
            role="button"
            tabIndex={0}
            onClick={() => onSelect(index)}
            onKeyDown={(event) => (event.key === 'Enter' || event.key === ' ') && onSelect(index)}
            className={cn(
              'group rounded-card border px-3 py-2 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
              index === active ? 'border-primary bg-primary-soft/40' : 'border-border hover:border-border-strong hover:bg-hover',
            )}
          >
            <div className="flex items-center gap-2 text-xs text-fg-muted">
              <span className="font-semibold text-fg">#{match.index}</span>
              <span className="tabular-nums">{t('matches.range', { start: match.start, end: match.end })}</span>
              <CopyButton text={match.text} className="ml-auto opacity-0 group-hover:opacity-100 focus-visible:opacity-100" />
            </div>
            <code className="mt-1 block font-mono text-[13px] break-all whitespace-pre-wrap text-fg" data-selectable>
              {match.text === '' ? <span className="text-fg-subtle italic">{t('matches.empty')}</span> : match.text}
            </code>
            {match.groups.length > 0 && (
              <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 border-t border-border pt-2 text-xs">
                {match.groups.map((group, i) => (
                  <div key={i} className="contents">
                    <dt className="font-mono text-fg-muted">{group?.name ?? `$${i + 1}`}</dt>
                    <dd className="font-mono break-all text-fg" data-selectable>
                      {group ? group.text : <span className="text-fg-subtle italic">{t('matches.unmatched')}</span>}
                    </dd>
                  </div>
                ))}
              </dl>
            )}
          </div>
        </li>
      ))}
    </ul>
  )
}
