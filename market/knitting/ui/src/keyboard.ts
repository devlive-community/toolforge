type KeyInfo = Pick<KeyboardEvent, 'code' | 'repeat' | 'defaultPrevented' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>

/** Repeated spaces are consumed without advancing or scrolling the page. */
export function counterKeyAction(event: KeyInfo, interactive: boolean): 'advance' | 'consume' | 'ignore' {
  if (event.code !== 'Space' || interactive || event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return 'ignore'
  return event.repeat ? 'consume' : 'advance'
}
