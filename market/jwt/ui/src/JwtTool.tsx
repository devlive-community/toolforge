import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { Lock, PenLine, ScanSearch } from 'lucide-react'
import { DecodeView } from './DecodeView'
import { JweView } from './JweView'
import { SignView } from './SignView'
import { SAMPLE_TOKEN } from './types'

export function JwtTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'decode' | 'jwe' | 'sign'>('decode')
  const [token, setToken] = useState(SAMPLE_TOKEN)
  useLaunchInput((text) => {
    setToken(text)
    setTab(text.trim().split('.').length === 5 ? 'jwe' : 'decode')
  })

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'decode', label: t('tabs.decode'), icon: <ScanSearch /> },
          { value: 'jwe', label: t('tabs.jwe'), icon: <Lock /> },
          { value: 'sign', label: t('tabs.sign'), icon: <PenLine /> },
        ]}
      />
      <div className="min-h-0 flex-1">
        {tab === 'decode' ? (
          <DecodeView token={token} onTokenChange={setToken} />
        ) : tab === 'jwe' ? (
          <JweView />
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
