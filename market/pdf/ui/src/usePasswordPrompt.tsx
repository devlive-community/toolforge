import { useRef, useState } from 'react'
import { Button, Input, Modal } from '@toolforge/ui'
import { usePlugin } from '@toolforge/plugin-ui-sdk'
import { fileName, type Info } from './types'

/** 打开 PDF；需要密码时弹窗询问，取消返回 null */
export function usePdfOpener() {
  const { t, call, errorMessage } = usePlugin()
  const [prompt, setPrompt] = useState<{ path: string; wrong: boolean } | null>(null)
  const [value, setValue] = useState('')
  const resolver = useRef<((password: string | null) => void) | null>(null)

  const ask = (path: string, wrong: boolean) =>
    new Promise<string | null>((resolve) => {
      resolver.current = resolve
      setValue('')
      setPrompt({ path, wrong })
    })
  const finish = (password: string | null) => {
    setPrompt(null)
    resolver.current?.(password)
    resolver.current = null
  }

  const open = async (path: string): Promise<{ info: Info; password: string | null } | { error: string }> => {
    let password: string | null = null
    for (;;) {
      try {
        const info = await call<Info>('inspect', { path, password })
        return { info, password }
      } catch (error) {
        const code = (error as { code?: string }).code
        if (code === 'pdf.password_required' || code === 'pdf.wrong_password') {
          password = await ask(path, code === 'pdf.wrong_password')
          if (password === null) return { error: '' }
          continue
        }
        return { error: errorMessage(error) }
      }
    }
  }

  const dialog = (
    <Modal
      open={prompt !== null}
      onOpenChange={(open) => !open && finish(null)}
      title={t('password.title')}
      description={prompt ? t(prompt.wrong ? 'password.wrong' : 'password.required', { name: fileName(prompt.path) }) : undefined}
      size="sm"
      closeLabel={t('common:close')}
      footer={
        <>
          <Button onClick={() => finish(null)}>{t('password.skip')}</Button>
          <Button variant="primary" disabled={!value} onClick={() => finish(value)}>
            {t('password.unlock')}
          </Button>
        </>
      }
    >
      <div className="px-5 pb-4">
        <Input type="password" autoFocus value={value} onChange={(e) => setValue(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && value && finish(value)} aria-label={t('password.title')} />
      </div>
    </Modal>
  )
  return { open, dialog }
}
