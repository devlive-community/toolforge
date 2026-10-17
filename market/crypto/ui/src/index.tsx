import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { KeyRound, LockKeyhole } from 'lucide-react'
import { RsaView } from './RsaView'
import { SymmetricView } from './SymmetricView'

function CryptoTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'symmetric' | 'rsa'>('symmetric')
  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'symmetric', label: t('tabs.symmetric'), icon: <LockKeyhole /> },
          { value: 'rsa', label: t('tabs.rsa'), icon: <KeyRound /> },
        ]}
        aria-label={t('name')}
      />
      {/* 两个页面都保持挂载：切换时保留密钥与内容，RSA 页也能接收剪贴板中的密钥 */}
      <div className={tab === 'symmetric' ? 'min-h-0 flex-1' : 'hidden'}>
        <SymmetricView />
      </div>
      <div className={tab === 'rsa' ? 'min-h-0 flex-1' : 'hidden'}>
        <RsaView onActivate={() => setTab('rsa')} />
      </div>
    </div>
  )
}

export default CryptoTool
