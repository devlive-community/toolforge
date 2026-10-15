import test from 'node:test'
import assert from 'node:assert/strict'
import { matchAssets, parseAssetLinks, parseReleaseFeed, releaseHtmlToMarkdown, loadReleaseData } from './release-data.mjs'

const repo = 'https://github.com/devlive-community/toolforge/releases'
const api = 'https://api.github.com/repos/devlive-community/toolforge/releases?per_page=15'
const response = (url, body, status = 200) => ({ url, status, ok: status === 200, text: async () => body, json: async () => JSON.parse(body) })
const quiet = () => {}
const page = '<relative-time datetime="2026-10-15T12:00:00Z"></relative-time><div data-test-selector="body-content"><h3>Features</h3><div><p>Real release notes.</p></div></div>'
const asset = '/devlive-community/toolforge/releases/download/v2.1.0/ToolForge_2.1.0_x64-setup.exe'
const entry = (tag) => `<entry><link href="${repo}/tag/${tag}"/><title>${tag}</title><updated>2026-10-15T12:00:00Z</updated><content type="html">&lt;p&gt;Notes&lt;/p&gt;</content></entry>`

test('matches installers without selecting signatures or updater archives', () => {
  const names = ['ToolForge_2.1.0_x64.dmg.sig', 'ToolForge_2.1.0_x64.app.tar.gz', 'ToolForge_2.1.0_aarch64.dmg', 'ToolForge_2.1.0_x64.dmg', 'ToolForge_2.1.0_arm64-setup.exe', 'ToolForge_2.1.0_amd64.AppImage', 'ToolForge_2.1.0_amd64.deb']
  const assets = matchAssets(names.map((name) => ({ name, browser_download_url: name })))
  assert.equal(assets.macosApple, names[2])
  assert.equal(assets.macosIntel, names[3])
  assert.equal(assets.windowsArm, names[4])
  assert.equal(assets.windowsX64, undefined)
  assert.equal(assets.linuxAppImage, names[5])
  assert.equal(assets.linuxDeb, names[6])
})

test('public asset parsing rejects other repositories, versions and origins', () => {
  const html = [`<a href="${asset}">ok</a>`, `<a href="${asset}">duplicate</a>`, `<a href="https://evil.example${asset}">bad</a>`, '<a href="/other/repo/releases/download/v2.1.0/file.exe">bad</a>', `<a href="${asset.replace('v2.1.0','v2.0.0')}">old</a>`].join('')
  const assets = parseAssetLinks(html, 'v2.1.0')
  assert.equal(assets.length, 1)
  assert.equal(assets[0].browser_download_url, 'https://github.com'+asset)
})

test('feed excludes future and prerelease tags and keeps real content', () => {
  const parsed = parseReleaseFeed(entry('v3.0.0') + entry('v2.2.0-beta.1') + entry('v2.1.0') + entry('v2.0.0'), 'v2.1.0')
  assert.deepEqual(parsed.map((item) => item.tag), ['v2.1.0','v2.0.0'])
  assert.equal(parsed[0].body, 'Notes')
})

test('markup conversion preserves headings and links without running raw HTML', () => {
  const body = releaseHtmlToMarkdown('<h3>Features</h3><ul><li><strong>core</strong>: fast &amp; local <a href="https://github.com/x/y">details</a></li></ul><script>bad()</script>')
  assert.match(body, /### Features/)
  assert.match(body, /- \*\*core\*\*: fast & local/)
  assert.match(body, /\[details\]\(https:\/\/github.com\/x\/y\)/)
  assert.doesNotMatch(body, /bad\(\)|<script>/)
})

test('API skips drafts and prereleases and uses the actual stable release', async () => {
  const items = [{tag_name:'v3.0.0',prerelease:true},{tag_name:'v2.2.0',draft:true},{tag_name:'v2.1.0',html_url:repo+'/tag/v2.1.0',assets:[],published_at:'2026-10-15T12:00:00Z',body:'real'}]
  const data = await loadReleaseData({ fetchImpl:async (url)=>response(url,JSON.stringify(items)), warn:quiet })
  assert.equal(data.version,'2.1.0')
  assert.equal(data.releases.length,1)
  assert.equal(data.fallback,undefined)
})

test('API rate limit recovers direct installers and notes from public GitHub', async () => {
  const calls = []
  const fetchImpl = async (url, options) => {
    calls.push({url,headers:options.headers})
    if (url===api) return response(url,'',403)
    if (url===repo+'/latest') return response(repo+'/tag/v2.1.0',page)
    if (url===repo+'/expanded_assets/v2.1.0') return response(url,`<a href="${asset}">download</a>`)
    if (url===repo+'.atom') return response(url,entry('v2.1.0')+entry('v2.0.0'))
    throw new Error('Unexpected URL '+url)
  }
  const data = await loadReleaseData({fetchImpl,token:'test-only-token',warn:quiet})
  assert.equal(data.version,'2.1.0')
  assert.equal(data.assets.windowsX64,'https://github.com'+asset)
  assert.match(data.releases[0].body,/Real release notes/)
  assert.equal(data.releases.length,2)
  assert.equal(calls[0].headers.Authorization,'Bearer test-only-token')
  assert.ok(calls.slice(1).every((call)=>!call.headers.Authorization))
})

test('missing feed still retains the authoritative latest page notes', async () => {
  const data=await loadReleaseData({warn:quiet,fetchImpl:async(url)=>{
    if(url===repo+'/latest') return response(repo+'/tag/v2.1.0',page)
    if(url===repo+'/expanded_assets/v2.1.0') return response(url,`<a href="${asset}">download</a>`)
    return response(url,'',403)
  }})
  assert.equal(data.releases.length,1)
  assert.match(data.releases[0].body,/Real release notes/)
})

test('complete network failure never presents a development version as latest', async () => {
  const data=await loadReleaseData({fetchImpl:async()=>{throw new Error('offline')},warn:quiet})
  assert.equal(data.fallback,true)
  assert.equal(data.version,'')
  assert.deepEqual(data.assets,{})
  assert.deepEqual(data.releases,[])
  assert.equal(data.url,repo+'/latest')
})


test('feed notes remain available when GitHub changes the release page markup', async () => {
  const data=await loadReleaseData({warn:quiet,fetchImpl:async(url)=>{
    if(url===repo+'/latest') return response(repo+'/tag/v2.1.0','<html>New layout</html>')
    if(url===repo+'/expanded_assets/v2.1.0') return response(url,`<a href="${asset}">download</a>`)
    if(url===repo+'.atom') return response(url,entry('v2.1.0'))
    return response(url,'',403)
  }})
  assert.equal(data.releases[0].body,'Notes')
  assert.equal(data.releases[0].date,'2026-10-15')
})
