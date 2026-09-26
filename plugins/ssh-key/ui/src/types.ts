export type Kind = 'ed25519' | 'ecdsa256' | 'ecdsa384' | 'ecdsa521' | 'rsa2048' | 'rsa3072' | 'rsa4096'

export interface Info {
  kind: 'public' | 'private'
  algorithm: string
  label: string
  bits: number
  comment: string
  encrypted: boolean
  cipher: string | null
  sha256: string
  md5: string
  randomart: string
  public: string
  options: string | null
  hosts: string | null
  line: number
  converted: string | null
}

export interface Generated {
  private: string
  public: string
  info: Info
}

export interface Inspection {
  keys: Info[]
  problems: { line: number; code: string }[]
}
