import { useRef, useState } from 'react'
import { Tabs } from '@toolforge/ui'
import { useLaunchInput, usePlugin, usePluginState } from '@toolforge/plugin-ui-sdk'
import { Barcode, ScanLine } from 'lucide-react'
import { Decode, type DecodeHandle } from './Decode'
import { Generate } from './Generate'
import { SAMPLES, type Format } from './types'

function BarcodeTool() {
  const { t } = usePlugin()
  const [tab, setTab] = useState<'generate' | 'decode'>('generate')
  const [format, setFormat] = usePluginState<Format>('format', 'ean13')
  const [text, setText] = usePluginState('text', SAMPLES.ean13)
  const decode = useRef<DecodeHandle>(null)
  // ⌘K 传入的内容只能取一次：在这里分发，复制的图片去识别，商品条码数字去生成
  useLaunchInput((input, label) => {
    if (label === 'image') {
      setTab('decode')
      decode.current?.paste()
      return
    }
    setTab('generate')
    setFormat(label === 'ean8' || label === 'upcA' ? label : 'ean13')
    setText(input.trim())
  })
  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <Tabs
        value={tab}
        onValueChange={setTab}
        className="self-start"
        items={[
          { value: 'generate', label: t('tabs.generate'), icon: <Barcode /> },
          { value: 'decode', label: t('tabs.decode'), icon: <ScanLine /> },
        ]}
        aria-label={t('name')}
      />
      {/* 两个页面都保持挂载：切换时保留状态，两边都能接收 ⌘K 传入的内容 */}
      <div className={tab === 'generate' ? 'min-h-0 flex-1' : 'hidden'}>
        <Generate format={format} setFormat={setFormat} text={text} setText={setText} />
      </div>
      <div className={tab === 'decode' ? 'min-h-0 flex-1' : 'hidden'}>
        <Decode ref={decode} />
      </div>
    </div>
  )
}

export default BarcodeTool
