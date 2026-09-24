import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { QrCode, ScanLine } from 'lucide-react'
import { Generate } from './Generate'
import { Decode } from './Decode'

function QrCodeTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'generate' | 'decode'>('generate')
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'generate', label: t('tabs.generate'), icon: <QrCode /> },
          { value: 'decode', label: t('tabs.decode'), icon: <ScanLine /> },
        ]}
        aria-label={t('name')}
      />
      <div className="min-h-0 flex-1">{tab === 'generate' ? <Generate /> : <Decode />}</div>
    </div>
  )
}

export default QrCodeTool
