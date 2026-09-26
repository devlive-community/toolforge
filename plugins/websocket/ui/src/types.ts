export type State = 'connecting' | 'open' | 'closing' | 'closed'
export type Direction = 'in' | 'out' | 'event'
export type SendKind = 'text' | 'binary' | 'ping'
export type Encoding = 'text' | 'hex' | 'base64'

export interface Message {
  seq: number
  ts: number
  direction: Direction
  kind: string
  size: number
  text: string | null
  truncated: boolean
  hex: string | null
  base64: string | null
  code: string | null
}

export interface Info {
  url: string
  address: string | null
  status: number | null
  protocol: string | null
  headers: [string, string][]
  handshakeMs: number | null
}

export interface Poll {
  messages: Message[]
  state: State
  info: Info
  last: number
}

export interface Snippet {
  name: string
  kind: SendKind
  encoding: Encoding
  payload: string
}

export interface Settings {
  url: string
  headers: [string, string][]
  protocols: string
  history: string[]
}

export const DEFAULT_SETTINGS: Settings = { url: 'wss://echo.websocket.org', headers: [], protocols: '', history: [] }
