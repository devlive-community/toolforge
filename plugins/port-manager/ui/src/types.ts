export type Protocol = 'tcp' | 'udp'

export interface Socket {
  protocol: Protocol
  localAddr: string
  port: number
  remoteAddr: string | null
  remotePort: number | null
  state: string
  pids: number[]
  loopback: boolean
}

export interface ProcessInfo {
  pid: number
  name: string
  exe: string | null
  command: string | null
  user: string | null
  memory: number
  startTime: number
}

export interface Listing {
  sockets: Socket[]
  processes: ProcessInfo[]
  total: number
}

export interface PortResult {
  port: number
  open: boolean
  ms: number | null
  reason: 'refused' | 'timeout' | 'unreachable' | 'other' | null
}

export interface CheckReport {
  host: string
  address: string
  results: PortResult[]
  open: number
  fakeIp: boolean
  elapsedMs: number
}
