import { useState } from 'react'
import { Checkbox, Switch, Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { Files, KeyRound, Type } from 'lucide-react'
import { FileHash } from './FileHash'
import { HmacText } from './HmacText'
import { TextHash } from './TextHash'
import { ALGORITHMS, ALGORITHM_LABELS, type Algorithm } from './types'

const DEFAULT_ALGORITHMS: Algorithm[] = ['md5', 'sha1', 'sha256', 'sha512']

export function HashTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'text' | 'hmac' | 'files'>('text')
  const [selected, setSelected] = useState<Algorithm[]>(DEFAULT_ALGORITHMS)
  const [uppercase, setUppercase] = useState(false)

  // 保持与 ALGORITHMS 相同的顺序
  const algorithms = ALGORITHMS.filter((a) => selected.includes(a))
  const toggle = (algorithm: Algorithm, on: boolean) =>
    setSelected((prev) => (on ? [...prev, algorithm] : prev.filter((a) => a !== algorithm)))

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <div className="flex flex-wrap items-center gap-3">
        <Tabs
          value={tab}
          onValueChange={setTab}
          items={[
            { value: 'text', label: t('tabs.text'), icon: <Type /> },
            { value: 'hmac', label: t('tabs.hmac'), icon: <KeyRound /> },
            { value: 'files', label: t('tabs.files'), icon: <Files /> },
          ]}
          aria-label={t('name')}
        />
        <div className="flex min-h-11 flex-1 flex-wrap items-center gap-x-4 gap-y-2 rounded-card border border-border bg-surface px-4 py-2 shadow-card">
          <Checkbox
            checked={algorithms.length === ALGORITHMS.length}
            indeterminate={algorithms.length > 0 && algorithms.length < ALGORITHMS.length}
            onCheckedChange={(on) => setSelected(on ? [...ALGORITHMS] : [])}
          >
            {t('allAlgorithms')}
          </Checkbox>
          <span className="h-4 w-px bg-border" />
          {ALGORITHMS.map((algorithm) => (
            <Checkbox key={algorithm} checked={selected.includes(algorithm)} onCheckedChange={(on) => toggle(algorithm, on)}>
              {ALGORITHM_LABELS[algorithm]}
            </Checkbox>
          ))}
          <label className="ml-auto flex items-center gap-2 text-[13px] text-fg-muted">
            {t('uppercase')}
            <Switch size="sm" checked={uppercase} onCheckedChange={setUppercase} aria-label={t('uppercase')} />
          </label>
        </div>
      </div>
      <div className="min-h-0 flex-1">
        {tab === 'text' && <TextHash algorithms={algorithms} uppercase={uppercase} />}
        {tab === 'hmac' && <HmacText algorithms={algorithms} uppercase={uppercase} />}
        {tab === 'files' && <FileHash algorithms={algorithms} uppercase={uppercase} />}
      </div>
    </div>
  )
}
