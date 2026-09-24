export type KeyEncoding = 'text' | 'base64'

export const ALGORITHMS = ['HS256', 'HS384', 'HS512', 'RS256', 'RS384', 'RS512', 'PS256', 'PS384', 'PS512', 'ES256', 'ES384', 'EdDSA'] as const
export type Algorithm = (typeof ALGORITHMS)[number]

export const isHmac = (alg: string | null | undefined) => !!alg && alg.startsWith('HS')

export interface TimeClaim {
  name: 'exp' | 'nbf' | 'iat'
  value: number
  iso: string | null
  relativeSeconds: number
}

export interface Decoded {
  algorithm: string | null
  header: string
  payload: string
  signature: string
  segments: { header: [number, number]; payload: [number, number]; signature: [number, number] }
  status: { times: TimeClaim[]; expired: boolean | null; notYetValid: boolean | null }
}

export interface Verified {
  valid: boolean
  algorithm: string
  reason: string | null
}

export const SAMPLE_TOKEN =
  'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c'
export const SAMPLE_SECRET = 'your-256-bit-secret'
