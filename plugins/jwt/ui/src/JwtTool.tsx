import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { PenLine, ScanSearch } from 'lucide-react'
import { DecodeView } from './DecodeView'
import { SignView } from './SignView'
import { SAMPLE_TOKEN } from './types'

export function JwtTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'decode' | 'sign'>('decode')
  const [token, setToken] = useState(SAMPLE_TOKEN)
  useLaunchInput((text) => {
    setToken(text)
    setTab('decode')
  })

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'decode', label: t('tabs.decode'), icon: <ScanSearch /> },
          { value: 'sign', label: t('tabs.sign'), icon: <PenLine /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        {tab === 'decode' ? (
          <DecodeView token={token} onTokenChange={setToken} />
        ) : (
          <SignView
            onOpen={(next) => {
              setToken(next)
              setTab('decode')
            }}
          />
        )}
      </div>
    </div>
  )
}
