import { useState, type KeyboardEvent } from 'react'
import { Button, Kbd, cn, toast } from '@toolforge/ui'
import { useErrorMessage } from '@toolforge/plugin-ui-sdk'
import { invoke } from '@tauri-apps/api/core'
import { useTranslation } from 'react-i18next'
import { DEFAULT_SHORTCUT, isMac } from '../lib/boot'

const MODIFIER_CODES = new Set(['MetaLeft', 'MetaRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'ShiftLeft', 'ShiftRight', 'CapsLock', 'Fn'])

const SYMBOLS: Record<string, string> = {
  Command: '⌘',
  Super: isMac ? '⌘' : 'Win',
  Control: isMac ? '⌃' : 'Ctrl',
  Alt: isMac ? '⌥' : 'Alt',
  Shift: isMac ? '⇧' : 'Shift',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  BracketLeft: '[',
  BracketRight: ']',
  Enter: '↵',
  Tab: '⇥',
  Backspace: '⌫',
}

/** 按键事件转为全局快捷键描述，例如 Alt+Space、Command+Shift+K；只有修饰键或缺少修饰键时返回 null */
export function toAccelerator(event: KeyboardEvent): string | null {
  if (MODIFIER_CODES.has(event.code)) return null
  const key = event.code.replace(/^Key/, '').replace(/^Digit/, '')
  const modifiers = [
    event.metaKey && (isMac ? 'Command' : 'Super'),
    event.ctrlKey && 'Control',
    event.altKey && 'Alt',
    event.shiftKey && 'Shift',
  ].filter((m): m is string => Boolean(m))
  const functionKey = /^F\d{1,2}$/.test(key)
  if (modifiers.length === 0 && !functionKey) return null
  return [...modifiers, key].join('+')
}

function Keys({ accelerator }: { accelerator: string }) {
  const { t } = useTranslation()
  return (
    <span className="flex items-center gap-1">
      {accelerator.split('+').map((part) => (
        <Kbd key={part} className="h-6 min-w-6 px-1.5 text-xs">
          {part === 'Space' ? t('keys.space') : (SYMBOLS[part] ?? part)}
        </Kbd>
      ))}
    </span>
  )
}

interface ShortcutRecorderProps {
  value: string | null
  onChange: (value: string | null) => Promise<void>
}

export function ShortcutRecorder({ value, onChange }: ShortcutRecorderProps) {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage()
  const [recording, setRecording] = useState(false)

  const apply = async (next: string | null) => {
    try {
      await onChange(next)
    } catch (error) {
      toast.error(errorMessage(error))
    }
  }

  // 录制时先暂停当前快捷键，否则按下它会被系统拦截，页面收不到按键
  const start = async () => {
    await invoke('launcher_set_shortcut', { shortcut: null }).catch(() => {})
    setRecording(true)
  }

  const cancel = () => {
    setRecording(false)
    invoke('launcher_set_shortcut', { shortcut: value }).catch(() => {})
  }

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!recording) return
    event.preventDefault()
    event.stopPropagation()
    if (event.code === 'Escape' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      cancel()
      return
    }
    const accelerator = toAccelerator(event)
    if (!accelerator) return
    setRecording(false)
    apply(accelerator)
  }

  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        onClick={recording ? cancel : start}
        onKeyDown={onKeyDown}
        onBlur={() => recording && cancel()}
        aria-label={t('settings.globalShortcut')}
        className={cn(
          'flex h-control-md min-w-40 items-center justify-center rounded-control border px-3 text-xs outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring',
          recording ? 'border-primary bg-primary-soft text-primary-fg' : 'border-border bg-surface text-fg-muted hover:bg-hover',
        )}
      >
        {recording ? t('settings.shortcutRecording') : value ? <Keys accelerator={value} /> : t('settings.shortcutOff')}
      </button>
      {!recording && value !== DEFAULT_SHORTCUT && (
        <Button size="sm" variant="ghost" onClick={() => apply(DEFAULT_SHORTCUT)}>
          {t('settings.shortcutReset')}
        </Button>
      )}
      {!recording && value && (
        <Button size="sm" variant="ghost" onClick={() => apply(null)}>
          {t('settings.shortcutDisable')}
        </Button>
      )}
    </div>
  )
}
