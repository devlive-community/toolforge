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
