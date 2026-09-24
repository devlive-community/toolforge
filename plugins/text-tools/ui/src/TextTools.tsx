import { useState } from 'react'
import { Button, CodeEditor, Panel, Tabs } from '@toolforge/ui'
import { host, usePlugin } from '@toolforge/plugin-ui-sdk'
import { BarChart3, CaseSensitive, ClipboardPaste, FileText, Rows3 } from 'lucide-react'
import { CasesView } from './CasesView'
import { LinesView } from './LinesView'
import { StatsView } from './StatsView'
import { SAMPLE } from './types'

type Tab = 'cases' | 'lines' | 'stats'

export function TextTools() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('cases')
  const [input, setInput] = useState(SAMPLE)

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'cases', label: t('tabs.cases'), icon: <CaseSensitive /> },
          { value: 'lines', label: t('tabs.lines'), icon: <Rows3 /> },
          { value: 'stats', label: t('tabs.stats'), icon: <BarChart3 /> },
        ]}
      />
      <div className="grid min-h-0 flex-1 grid-cols-2 gap-3">
        <Panel
          icon={<FileText />}
          title={t('input')}
          actions={
            <>
              <Button size="sm" onClick={() => setInput('')}>
                {t('common:clear')}
              </Button>
              <Button size="sm" onClick={() => host.clipboard.readText().then(setInput).catch(() => {})}>
                <ClipboardPaste />
                {t('common:paste')}
              </Button>
            </>
          }
        >
          <CodeEditor value={input} onChange={setInput} language="text" lineWrapping placeholder={t('placeholder')} aria-label={t('input')} />
        </Panel>
        <Panel title={t(`tabs.${tab}`)} bodyClassName={tab === 'lines' ? undefined : 'overflow-auto'}>
          {tab === 'cases' && <CasesView input={input} />}
          {tab === 'lines' && <LinesView input={input} />}
          {tab === 'stats' && <StatsView input={input} />}
        </Panel>
      </div>
    </div>
  )
}
