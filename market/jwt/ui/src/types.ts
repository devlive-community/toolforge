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

export interface Decrypted {
  header: unknown
  parts: [number, number, number, number, number]
  alg: string
  enc: string
  encryptedKeyLen: number
  plaintext: string
  kind: string
  plaintextBase64: string
}

export interface ParsedJwe {
  header: unknown
  parts: [number, number, number, number, number]
  alg: string
  enc: string
  encryptedKeyLen: number
}

export type JweKeyEncoding = 'hex' | 'base64url'

export const SAMPLE_JWE =
  'eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIn0..BwcHBwcHBwcHBwcH.Zw_KMAAvvYOj.09Bj38RM1qHx6ecwYHlJmA'
export const SAMPLE_JWE_KEY_HEX = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f'
