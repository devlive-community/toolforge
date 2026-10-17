export type Encoding = 'utf8' | 'hex' | 'base64'
export type Algorithm = 'aes' | 'sm4' | 'chacha20'
export type Mode = 'gcm' | 'cbc' | 'ctr' | 'ecb'
export type Direction = 'encrypt' | 'decrypt'
export type KeyEncoding = 'hex' | 'base64' | 'utf8' | 'password'
export type Padding = 'pkcs7' | 'zero' | 'none'

export interface SymmetricOutput {
  output: string
  iv: string | null
  salt: string | null
  derivedKey: string | null
  bytes: number
}

export type RsaPadding = 'oaep-sha256' | 'oaep-sha1' | 'pkcs1v15'
export type Scheme = 'pkcs1v15' | 'pss'
export type Hash = 'sha256' | 'sha384' | 'sha512'
export type Operation = 'encrypt' | 'decrypt' | 'sign' | 'verify'

export interface KeyInfo {
  kind: 'private' | 'public'
  bits: number
  publicKey: string
}

export interface KeyPair {
  privateKey: string
  publicKey: string
  bits: number
}

export type Sm2Operation = 'sign' | 'verify' | 'convert'
export type SignatureFormat = 'der' | 'raw'
export type SignatureInput = 'auto' | SignatureFormat
export type BinaryEncoding = 'hex' | 'base64'
export type UserIdEncoding = 'utf8' | 'hex'

export interface Sm2KeyInfo {
  kind: 'private' | 'public'
  format: string
  publicKey: string
  publicKeyPem: string
}

export interface Sm2KeyPair {
  privateKey: string
  publicKey: string
}

export interface Sm2Details {
  userId: string
  entla: number
  za: string
  e: string
  messageBytes: number
  messagePreview: string
  publicKey: string
}

export interface Sm2SignOutput {
  signature: string
  format: SignatureFormat
  r: string
  s: string
  details: Sm2Details
}

export interface Sm2VerifyOutput {
  valid: boolean
  format: SignatureFormat
  r: string
  s: string
  details: Sm2Details
}

export interface Sm2ConvertOutput {
  signature: string
  from: SignatureFormat
  r: string
  s: string
}

/** 剪贴板识别出的密钥，seq 递增表示新的一次输入 */
export interface LaunchKey {
  seq: number
  label: string | null
  text: string
}
