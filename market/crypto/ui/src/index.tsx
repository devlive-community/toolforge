import { useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin } from '@toolforge/plugin-ui-sdk'
import { KeyRound, LockKeyhole, Signature } from 'lucide-react'
import { RsaView } from './RsaView'
import { SymmetricView } from './SymmetricView'
import { Sm2View } from './Sm2View'
import type { LaunchKey } from './types'

type Tab = 'symmetric' | 'rsa' | 'sm2'

function CryptoTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<Tab>('symmetric')
  const [launch, setLaunch] = useState<LaunchKey | null>(null)

  // 剪贴板中的 PEM 密钥：按识别结果切到 RSA 或 SM2 页并填入
  useLaunchInput((text, label) => {
    setTab(label?.startsWith('sm2') ? 'sm2' : 'rsa')
    setLaunch((prev) => ({ seq: (prev?.seq ?? 0) + 1, label, text }))
  })

  return (
    <div className="flex h-full min-h-0 flex-col gap-4">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'symmetric', label: t('tabs.symmetric'), icon: <LockKeyhole /> },
          { value: 'rsa', label: t('tabs.rsa'), icon: <KeyRound /> },
          { value: 'sm2', label: t('tabs.sm2'), icon: <Signature /> },
        ]}
        aria-label={t('name')}
      />
      {/* 各页面都保持挂载：切换时保留密钥与内容 */}
      <div className={tab === 'symmetric' ? 'min-h-0 flex-1' : 'hidden'}>
        <SymmetricView />
      </div>
      <div className={tab === 'rsa' ? 'min-h-0 flex-1' : 'hidden'}>
        <RsaView launch={launch} />
      </div>
      <div className={tab === 'sm2' ? 'min-h-0 flex-1' : 'hidden'}>
        <Sm2View launch={launch} />
      </div>
    </div>
  )
}

export default CryptoTool
