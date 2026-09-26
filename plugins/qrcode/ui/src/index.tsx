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
      {/* 两个页面都保持挂载：切换时保留状态，识别页也能接收剪贴板图片 */}
      <div className={tab === 'generate' ? 'min-h-0 flex-1' : 'hidden'}>
        <Generate />
      </div>
      <div className={tab === 'decode' ? 'min-h-0 flex-1' : 'hidden'}>
        <Decode onActivate={() => setTab('decode')} />
      </div>
    </div>
  )
}

export default QrCodeTool
