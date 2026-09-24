import type { AppError } from '@toolforge/plugin-ui-sdk'

export const ALGORITHMS = ['md5', 'sha1', 'sha224', 'sha256', 'sha384', 'sha512', 'sha3_256', 'sha3_512', 'sm3', 'crc32'] as const
export type Algorithm = (typeof ALGORITHMS)[number]

export const ALGORITHM_LABELS: Record<Algorithm, string> = {
  md5: 'MD5',
  sha1: 'SHA-1',
  sha224: 'SHA-224',
  sha256: 'SHA-256',
  sha384: 'SHA-384',
  sha512: 'SHA-512',
  sha3_256: 'SHA3-256',
  sha3_512: 'SHA3-512',
  sm3: 'SM3',
  crc32: 'CRC32',
}

export interface TextReport {
  results: { algorithm: Algorithm; digest: string }[]
  bytes: number
  elapsedMs: number
}

export interface FileResult {
  path: string
  name: string
  size: number
  digests: Partial<Record<Algorithm, string>>
  error: AppError | null
}

export interface FilesReport {
  files: FileResult[]
  totalBytes: number
  elapsedMs: number
}

export type KeyEncoding = 'text' | 'hex' | 'base64'
export type OutputEncoding = 'hex' | 'base64'

export interface HmacReport {
  results: { algorithm: Algorithm; mac: string; matches: boolean | null }[]
  keyBytes: number
  elapsedMs: number
}
