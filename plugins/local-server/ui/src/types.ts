export interface Settings {
  root: string
  port: number
  lan: boolean
  listing: boolean
  spa: boolean
  cors: boolean
  hideDotfiles: boolean
}

export const DEFAULT_SETTINGS: Settings = {
  root: '',
  port: 8000,
  lan: false,
  listing: true,
  spa: false,
  cors: false,
  hideDotfiles: true,
}

export interface Address {
  url: string
  kind: 'local' | 'lan'
  interface: string | null
  qr: string | null
}

export interface Addresses {
  addresses: Address[]
  available: boolean
}
