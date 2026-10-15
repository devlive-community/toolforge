import type { Metadata } from 'next'
import Link from 'next/link'
import { ArrowRight, Check, Code2, FileText, ImageIcon, Monitor, Puzzle, ShieldCheck } from 'lucide-react'
import { DownloadButtons } from '@/components/DownloadButtons'
import { QuickDownloads } from '@/components/QuickDownloads'
import { HomeProductPreview } from '@/components/HomeProductPreview'
import { catalog, getTool, REPO_URL, toolName, toolStatus, type Lang } from '@/lib/catalog'
import { homeCopy } from '@/lib/home-copy'
import styles from './home.module.css'

export async function generateMetadata({ params }: { params: Promise<{ lang: string }> }): Promise<Metadata> {
  const { lang } = await params
  const copy = homeCopy[lang as Lang]
  const title = lang === 'en' ? 'ToolForge — The desktop toolbox that stays local' : 'ToolForge — 桌面工具箱，全部本地运行'
  return {
    title,
    description: copy.description,
    openGraph: {
      title, description: copy.description, type: 'website', siteName: 'ToolForge',
      url: `https://toolforge.devlive.org/${lang}/`,
      images: [{ url: '/og-image.png', width: 1200, height: 630, alt: 'ToolForge' }],
    },
    twitter: { card: 'summary_large_image', title, description: copy.description, images: ['/og-image.png'] },
  }
}

export default async function HomePage({ params }: { params: Promise<{ lang: string }> }) {
  const { lang: rawLang } = await params
  const lang = rawLang as Lang
  const copy = homeCopy[lang]
  const categoryCount = new Set(catalog.map((tool) => tool.category)).size

  return (
    <div className={styles.home}>
      <section className={`${styles.wrap} ${styles.hero}`} aria-labelledby="home-title">
        <div className={styles.heroCopy}>
          <p className={styles.badge}><span />{copy.badge}</p>
          <h1 id="home-title" className={styles.title}>{copy.title}<br />{copy.titleSecond}<span>{copy.titleAccent}</span></h1>
          <p className={styles.subtitle}>{copy.subtitle}<br />{copy.subtitleSecond}</p>
          <QuickDownloads lang={lang} />
          <div className={styles.actions}>
            <Link href={`/${lang}/tools/`} className={styles.secondary}>{copy.browse}<ArrowRight size={17} /></Link>
          </div>
        </div>
        <div className={styles.heroVisual}><HomeProductPreview lang={lang} toolCount={catalog.length} /></div>
      </section>

      <section className={`${styles.wrap} ${styles.proof}`} aria-label={copy.why}>
        <div className={styles.proofItem}><Monitor /><div><h2>{copy.localTitle}</h2><p>{copy.localBody}</p></div></div>
        <div className={styles.proofItem}><Puzzle /><div><h2>{copy.pluginTitle}</h2><p>{copy.pluginBody}</p></div></div>
        <a href={REPO_URL} target="_blank" rel="noreferrer" className={styles.proofItem}><ShieldCheck /><div><h2>{copy.openTitle}<ArrowRight size={13} /></h2><p>{copy.openBody}</p></div></a>
      </section>

      <section className={`${styles.wrap} ${styles.workflows}`} aria-labelledby="workflow-title">
        <div className={styles.sectionHeading}>
          <p className={styles.eyebrow}>BUILT FOR YOUR EVERYDAY</p>
          <h2 id="workflow-title">{copy.workflowTitle}</h2>
          <p>{copy.workflowBody}</p>
        </div>
        <div className={styles.workflowGrid}>
          <article className={`${styles.scenario} ${styles.development}`}>
            <p className={styles.scenarioLabel}><Code2 size={18} />{copy.development}</p>
            <h3>{copy.developmentTitle}</h3>
            <p className={styles.scenarioBody}>{copy.developmentBody}</p>
            <div className={styles.codeDemo} role="img" aria-label={copy.jsonPreview}>
              <div><div className={styles.demoToolbar}><span>{copy.jsonInput}</span><span className={styles.codeDot} /></div><pre><code>{'{"name":"ToolForge","local":true,"platforms":["macOS","Windows","Linux"]}'}</code></pre></div>
              <div><div className={styles.demoToolbar}><span>{copy.jsonOutput}</span><Check size={14} /></div><pre><code>{'{\n  '}<span className={styles.codeKey}>{'"name"'}</span>{': '}<span className={styles.codeValue}>{'"ToolForge"'}</span>{',\n  '}<span className={styles.codeKey}>{'"local"'}</span>{': '}<span className={styles.codeBoolean}>true</span>{',\n  '}<span className={styles.codeKey}>{'"platforms"'}</span>{': [\n    '}<span className={styles.codeValue}>{'"macOS"'}</span>{',\n    '}<span className={styles.codeValue}>{'"Windows"'}</span>{',\n    '}<span className={styles.codeValue}>{'"Linux"'}</span>{'\n  ]\n}'}</code></pre></div>
            </div>
            <ScenarioLinks lang={lang} slugs={['json-formatter', 'format-converter', 'timestamp']} />
          </article>

          <article className={styles.scenario}>
            <p className={styles.scenarioLabel}><ImageIcon size={18} />{copy.images}</p>
            <h3>{copy.imagesTitle}</h3>
            <div className={styles.imageDemo} role="img" aria-label={copy.imagePreview}>
              <div className={styles.imageStack}><Landscape /><Landscape /><Landscape /></div>
              <ArrowRight size={19} className={styles.imageArrow} />
              <div className={styles.checker}><Landscape transparent /></div>
              <div className={`${styles.checker} ${styles.smallImage}`}><Landscape transparent /></div>
            </div>
            <div className={styles.progressLabel}><span>{copy.imageDone}</span><span>3 / 3<Check size={12} /></span></div>
            <div className={styles.progressTrack}><span /></div>
            <ScenarioLinks lang={lang} slugs={['ocr', 'image-compressor', 'background-remover']} />
          </article>

          <article className={styles.scenario}>
            <p className={styles.scenarioLabel}><FileText size={18} />{copy.files}</p>
            <h3>{copy.filesTitle}</h3>
            <div className={styles.fileDemo} role="img" aria-label={copy.renamePreview}>
              <div className={styles.fileHead}><span>{copy.fileBefore}</span><span /><span>{copy.fileAfter}</span></div>
              {['01', '02', '03'].map((number) => <div className={styles.fileRow} key={number}><span><FileText size={13} />IMG_0{number}.png</span><ArrowRight size={13} /><span>project_{number}.png</span></div>)}
            </div>
            <ScenarioLinks lang={lang} slugs={['batch-rename', 'text-diff']} />
          </article>
        </div>
        <p className={styles.demoNote}>{copy.demoNote}</p>
      </section>

      <section id="download" className={`${styles.wrap} ${styles.downloadSection}`} aria-labelledby="download-title">
        <div className={styles.downloadPanel}>
          <div className={styles.downloadIntro}>
            <p className={styles.eyebrow}>READY WHEN YOU ARE</p>
            <h2 id="download-title">{copy.closingTitle}</h2>
            <p>{copy.closingBody.replace('{categories}', String(categoryCount))}</p>
            <Link className={styles.browseLink} href={`/${lang}/tools/`}>{copy.browse}<ArrowRight size={16} /></Link>
          </div>
          <div className={styles.downloadChoices}>
            <p className={styles.downloadLabel}>{copy.choosePlatform}</p>
            <DownloadButtons lang={lang} size="sm" />
          </div>
        </div>
      </section>
    </div>
  )
}

function ScenarioLinks({ lang, slugs }: { lang: Lang; slugs: string[] }) {
  const tools = slugs.map(getTool).filter((tool) => tool && (toolStatus(tool) !== 'upcoming' || tool.updateOnly))
  return <div className={styles.scenarioLinks}>{tools.map((tool) => tool && <Link href={`/${lang}/tools/${tool.slug}/`} key={tool.slug}>{toolName(tool, lang)}<ArrowRight size={12} /></Link>)}</div>
}

function Landscape({ transparent = false }: { transparent?: boolean }) {
  return <svg viewBox="0 0 120 105" fill="none" aria-hidden="true" focusable="false">{!transparent && <rect width="120" height="105" fill="#c7dfcd" />}<circle cx="80" cy="30" r="14" fill="#f2bc65" /><path d="M0 91 35 40 75 105H0Z" fill="#39765b" /><path d="m36 105 47-68 37 51v17Z" fill="#72a884" /><path d="m21 59 14-19 14 23-13-5-7 5Z" fill="#edf5eb" /></svg>
}
