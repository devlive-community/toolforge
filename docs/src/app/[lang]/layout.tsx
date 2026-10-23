import type { Metadata, Viewport } from 'next'
import { notFound } from 'next/navigation'
import { Footer } from '@/components/Footer'
import { Header } from '@/components/Header'
import { LANGS, type Lang } from '@/lib/catalog'
import { siteCopy } from '@/lib/site-copy'
import '../../styles/globals.css'

const themeScript = `(function(){try{var t=localStorage.getItem('tf-theme');var d=t==='dark'||(t!=='light'&&matchMedia('(prefers-color-scheme: dark)').matches);document.documentElement.classList.toggle('dark',d)}catch(e){document.documentElement.classList.toggle('dark',matchMedia('(prefers-color-scheme: dark)').matches)}})()`

export function generateStaticParams() {
  return LANGS.map((lang) => ({ lang }))
}

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params
  const zh = lang !== 'en'
  const title = 'ToolForge — ' + (zh ? '本地优先的桌面工具箱' : 'Your local-first desktop toolkit')
  const description = zh
    ? '面向开发与日常效率的桌面工具箱：识别、转换、编码、网络与系统工具，本地处理，插件架构。'
    : 'A desktop toolbox for development and everyday productivity: OCR, converters, encoding, network and system tools, with local processing and independent plugins.'
  const ogImage = { url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }
  return {
    title,
    description,
    metadataBase: new URL('https://toolforge.devlive.org'),
    alternates: { canonical: `/${zh ? 'zh' : 'en'}/` },
    icons: {
      icon: [
        { url: '/favicon.ico', sizes: '48x48' },
        { url: '/icon-192.png', sizes: '192x192', type: 'image/png' },
      ],
      apple: '/apple-touch-icon.png',
    },
    openGraph: {
      title,
      description,
      type: 'website',
      siteName: 'ToolForge',
      url: `https://toolforge.devlive.org/${zh ? 'zh' : 'en'}/`,
      images: [ogImage],
    },
    twitter: { card: 'summary_large_image', title, description, images: ['/og-image.png'] },
  }
}

export const viewport: Viewport = {
  themeColor: [
    { media: '(prefers-color-scheme: dark)', color: '#0c110f' },
    { media: '(prefers-color-scheme: light)', color: '#f7f9f7' },
  ],
}

export default async function RootLayout({ children, params }: { children: React.ReactNode; params: Promise<{ lang: string }> }) {
  const { lang } = await params
  if (!LANGS.includes(lang as Lang)) notFound()
  return (
    <html lang={lang} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: themeScript }} />
      </head>
      <body className="site min-h-dvh">
        <a className="site-skip" href="#main-content">
          {siteCopy[lang as Lang].skip}
        </a>
        <Header lang={lang as Lang} />
        <main id="main-content">{children}</main>
        <Footer lang={lang as Lang} />
      </body>
    </html>
  )
}
