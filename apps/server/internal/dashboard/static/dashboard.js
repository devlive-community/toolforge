// ToolForge plugin statistics dashboard. Reads /v1/dashboard every 30 seconds.
// Counts arrive as decimal strings and are handled with BigInt to keep precision.
// All text is inserted with textContent; no HTML is built from data.
;(() => {
  'use strict'
  const REFRESH_MS = 30000
  const zh = (navigator.language || 'zh').toLowerCase().startsWith('zh')
  const lang = zh ? 'zh-CN' : 'en-US'
  const T = zh
    ? {
        title: 'ToolForge 插件数据',
        subtitle: '官方在线插件的下载情况',
        loading: '正在加载…',
        total: '累计下载',
        today: '今日下载',
        week: '近 7 天',
        weekHint: '最近 7 天合计',
        month: '近 30 天',
        monthHint: '最近 30 天合计',
        plugins: '在线插件',
        downloaded: '有下载的插件',
        trend: '近 30 天下载趋势',
        timezone: '北京时间',
        ranking: '插件下载排行',
        versions: '热门版本',
        plugin: '插件',
        version: '版本',
        downloads: '下载',
        recent: '最近下载',
        liveTag: '实时',
        note: '下载量为客户端完整下载并通过大小、SHA-256 与官方签名校验后确认的次数，不等于设备数或安装量。',
        since: (d) => `自 ${d} 起累计`,
        updated: (t) => `更新于 ${t} · 每 30 秒刷新`,
        failed: '数据暂时不可用，稍后自动重试',
        versionsCount: (n) => `${n} 个已发布版本`,
        coverage: (p) => `占在线插件 ${p}`,
        vsYesterday: (d) => `较昨日 ${d}`,
        noData: '暂无数据',
        justNow: '刚刚',
        minutes: (n) => `${n} 分钟前`,
        hours: (n) => `${n} 小时前`,
        days: (n) => `${n} 天前`,
        daily: (date, n) => `${date}：${n} 次下载`,
      }
    : {
        title: 'ToolForge plugin statistics',
        subtitle: 'Downloads of official online plugins',
        loading: 'Loading…',
        total: 'Total downloads',
        today: 'Today',
        week: 'Last 7 days',
        weekHint: 'Sum of the last 7 days',
        month: 'Last 30 days',
        monthHint: 'Sum of the last 30 days',
        plugins: 'Online plugins',
        downloaded: 'Downloaded plugins',
        trend: 'Downloads in the last 30 days',
        timezone: 'Beijing time',
        ranking: 'Plugin ranking',
        versions: 'Popular versions',
        plugin: 'Plugin',
        version: 'Version',
        downloads: 'Downloads',
        recent: 'Recent downloads',
        liveTag: 'Live',
        note: 'A download counts once the client fully downloads a package and verifies its size, SHA-256 and official signature. It is not a count of devices or installations.',
        since: (d) => `Counting since ${d}`,
        updated: (t) => `Updated ${t} · refreshes every 30 s`,
        failed: 'Data is temporarily unavailable; retrying automatically',
        versionsCount: (n) => `${n} published versions`,
        coverage: (p) => `${p} of online plugins`,
        vsYesterday: (d) => `${d} vs yesterday`,
        noData: 'No data yet',
        justNow: 'just now',
        minutes: (n) => `${n} min ago`,
        hours: (n) => `${n} h ago`,
        days: (n) => `${n} d ago`,
        daily: (date, n) => `${date}: ${n} downloads`,
      }

  document.documentElement.lang = lang
  document.title = T.title
  for (const el of document.querySelectorAll('[data-i18n]')) {
    const value = T[el.dataset.i18n]
    if (typeof value === 'string') el.textContent = value
  }

  const $ = (id) => document.getElementById(id)
  const big = (s) => {
    try {
      return BigInt(s)
    } catch {
      return 0n
    }
  }
  const fmt = (n) => (typeof n === 'bigint' ? n : big(String(n))).toLocaleString(lang)
  const el = (tag, className, text) => {
    const node = document.createElement(tag)
    if (className) node.className = className
    if (text !== undefined) node.textContent = text
    return node
  }
  const pluginName = (info, id) => {
    const names = info[id] && info[id].names
    if (names) return names[lang] || names['en-US'] || names['zh-CN'] || id
    return id.replace(/^org\.devlive\.toolforge\./, '')
  }
  const relative = (iso) => {
    const seconds = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
    if (seconds < 60) return T.justNow
    if (seconds < 3600) return T.minutes(Math.floor(seconds / 60))
    if (seconds < 86400) return T.hours(Math.floor(seconds / 3600))
    return T.days(Math.floor(seconds / 86400))
  }

  function renderKpis(d) {
    const daily = d.daily.map((x) => big(x.downloads))
    const sum = (list) => list.reduce((a, b) => a + b, 0n)
    const today = daily[daily.length - 1] ?? 0n
    const yesterday = daily[daily.length - 2] ?? 0n
    $('kpi-total').textContent = fmt(d.totalDownloads)
    $('kpi-since').textContent = T.since(new Date(d.countedSince).toLocaleDateString(lang))
    $('kpi-today').textContent = fmt(today)
    const delta = today - yesterday
    const deltaEl = $('kpi-today-delta')
    deltaEl.textContent = T.vsYesterday((delta > 0n ? '+' : '') + fmt(delta))
    deltaEl.className = 'hint ' + (delta > 0n ? 'up' : delta < 0n ? 'down' : '')
    $('kpi-week').textContent = fmt(sum(daily.slice(-7)))
    $('kpi-month').textContent = fmt(sum(daily))
    $('kpi-plugins').textContent = fmt(d.plugins)
    $('kpi-versions').textContent = T.versionsCount(fmt(d.versions))
    $('kpi-downloaded').textContent = fmt(d.downloadedPlugins)
    $('kpi-coverage').textContent = d.plugins > 0 ? T.coverage(Math.round((d.downloadedPlugins / d.plugins) * 100) + '%') : ''
  }

  const SVG = 'http://www.w3.org/2000/svg'
  const svgEl = (tag, attrs) => {
    const node = document.createElementNS(SVG, tag)
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v))
    return node
  }

  let hoverIndex = null
  let pinnedIndex = null

  function renderTrend(daily) {
    const box = $('trend')
    const width = Math.max(320, box.clientWidth)
    const height = Math.max(220, box.clientHeight)
    const pad = { top: 16, right: 12, bottom: 26, left: 40 }
    const values = daily.map((x) => Number(big(x.downloads)))
    const max = Math.max(1, ...values)
    const step = niceStep(max)
    const top = Math.ceil(max / step) * step
    const plotW = width - pad.left - pad.right
    const plotH = height - pad.top - pad.bottom
    const x = (i) => pad.left + (plotW * (i + 0.5)) / values.length
    const y = (v) => pad.top + plotH - (plotH * v) / top
    const svg = svgEl('svg', { viewBox: `0 0 ${width} ${height}`, preserveAspectRatio: 'none' })
    const defs = svgEl('defs', {})
    defs.append(gradient('barFill', '#3ddc84', 0.85, '#24b366', 0.35), gradient('areaFill', '#3ddc84', 0.25, '#3ddc84', 0))
    svg.append(defs)
    for (let v = 0; v <= top; v += step) {
      svg.append(svgEl('line', { x1: pad.left, x2: width - pad.right, y1: y(v), y2: y(v), class: 'grid-line' }))
      const label = svgEl('text', { x: pad.left - 8, y: y(v) + 4, 'text-anchor': 'end', class: 'axis' })
      label.textContent = v.toLocaleString(lang)
      svg.append(label)
    }
    const barW = Math.max(3, (plotW / values.length) * 0.56)
    const bars = values.map((v, i) => {
      const bar = svgEl('rect', { x: x(i) - barW / 2, y: y(v), width: barW, height: Math.max(0, pad.top + plotH - y(v)), rx: 3, class: 'bar' })
      svg.append(bar)
      return bar
    })
    const points = values.map((v, i) => `${x(i)},${y(v)}`)
    svg.append(svgEl('path', { d: `M${x(0)},${pad.top + plotH} L${points.join(' L')} L${x(values.length - 1)},${pad.top + plotH} Z`, class: 'area' }))
    svg.append(svgEl('polyline', { points: points.join(' '), class: 'line' }))
    const last = values.length - 1
    svg.append(svgEl('circle', { cx: x(last), cy: y(values[last]), r: 4, class: 'dot' }))
    const every = values.length > 20 ? 5 : 2
    daily.forEach((d, i) => {
      if (i % every !== (values.length - 1) % every) return
      const label = svgEl('text', { x: x(i), y: height - 6, 'text-anchor': 'middle', class: 'axis' })
      label.textContent = d.date.slice(5)
      svg.append(label)
    })
    // 悬停显示详情，点击把详情钉住；命中区覆盖整个列，0 高度的柱也能选中
    hoverIndex = null
    const hitW = plotW / values.length
    values.forEach((_, i) => {
      const hit = svgEl('rect', { x: pad.left + hitW * i, y: pad.top, width: hitW, height: plotH, class: 'hit' })
      hit.addEventListener('pointerenter', () => {
        hoverIndex = i
        syncTip()
      })
      hit.addEventListener('pointerleave', () => {
        if (hoverIndex === i) {
          hoverIndex = null
          syncTip()
        }
      })
      hit.addEventListener('click', () => {
        pinnedIndex = pinnedIndex === i ? null : i
        syncTip()
      })
      svg.append(hit)
    })
    const tip = el('div', 'chart-tip')
    tip.hidden = true
    const syncTip = () => {
      const index = hoverIndex ?? pinnedIndex
      bars.forEach((bar, i) => {
        bar.classList.toggle('pinned', pinnedIndex === i)
        bar.classList.toggle('active', hoverIndex === i)
      })
      if (index === null) {
        tip.hidden = true
        return
      }
      tip.textContent = T.daily(daily[index].date, values[index].toLocaleString(lang))
      tip.style.left = Math.min(width - 80, Math.max(80, x(index))) + 'px'
      tip.style.top = Math.max(30, y(values[index])) + 'px'
      tip.hidden = false
    }
    box.replaceChildren(svg, tip)
    syncTip()
    box.setAttribute('aria-label', T.trend)
  }
  function gradient(id, from, fromOpacity, to, toOpacity) {
    const g = svgEl('linearGradient', { id, x1: 0, y1: 0, x2: 0, y2: 1 })
    g.append(svgEl('stop', { offset: '0%', 'stop-color': from, 'stop-opacity': fromOpacity }), svgEl('stop', { offset: '100%', 'stop-color': to, 'stop-opacity': toOpacity }))
    return g
  }
  function niceStep(max) {
    const raw = max / 4
    const power = 10 ** Math.floor(Math.log10(raw || 1))
    for (const m of [1, 2, 5, 10]) if (raw <= m * power) return Math.max(1, m * power)
    return Math.max(1, 10 * power)
  }

  function renderRanking(d) {
    const list = $('ranking')
    const total = big(d.totalDownloads)
    const top = d.ranking.slice(0, 10)
    if (top.length === 0) return list.replaceChildren(el('li', 'empty', T.noData))
    const max = big(top[0].downloads) || 1n
    list.replaceChildren(
      ...top.map((item, i) => {
        const n = big(item.downloads)
        const li = el('li')
        const count = el('span', 'count', fmt(n))
        if (total > 0n) count.append(el('span', 'share', ((Number((n * 10000n) / total) / 100).toFixed(1)) + '%'))
        const track = el('div', 'track')
        const fill = el('div', 'fill')
        fill.style.width = Number((n * 1000n) / max) / 10 + '%'
        track.append(fill)
        li.append(el('span', 'rank', String(i + 1)), el('span', 'name', pluginName(d.pluginInfo, item.pluginId)), count, track)
        li.title = item.pluginId
        return li
      }),
    )
  }

  function renderVersions(d) {
    const body = $('versions')
    if (d.topVersions.length === 0) {
      const row = el('tr')
      const cell = el('td', 'empty', T.noData)
      cell.colSpan = 3
      row.append(cell)
      return body.replaceChildren(row)
    }
    body.replaceChildren(
      ...d.topVersions.map((item) => {
        const row = el('tr')
        const version = el('td')
        version.append(el('code', '', 'v' + item.version))
        row.append(el('td', '', pluginName(d.pluginInfo, item.pluginId)), version, el('td', 'num', fmt(item.downloads)))
        return row
      }),
    )
  }

  function renderRecent(d) {
    const list = $('recent')
    if (d.recent.length === 0) return list.replaceChildren(el('li', 'empty', T.noData))
    list.replaceChildren(
      ...d.recent.slice(0, 8).map((item) => {
        const li = el('li')
        const name = el('span', 'name', pluginName(d.pluginInfo, item.pluginId))
        name.append(' ', el('code', '', 'v' + item.version))
        const time = el('time', '', relative(item.at))
        time.dateTime = item.at
        li.append(el('span', 'dot'), name, time)
        return li
      }),
    )
  }

  let latest = null
  async function load() {
    const status = document.querySelector('.status')
    try {
      const response = await fetch('/v1/dashboard', { cache: 'no-store' })
      if (!response.ok) throw new Error(String(response.status))
      latest = await response.json()
      latest.pluginInfo = latest.pluginInfo || {}
      renderKpis(latest)
      renderTrend(latest.daily)
      renderRanking(latest)
      renderVersions(latest)
      renderRecent(latest)
      // 首次渲染成功才撤掉骨架；之后刷新失败保留旧数据，只在状态区提示
      document.body.classList.remove('loading', 'load-error')
      status.classList.remove('error')
      $('updated').textContent = T.updated(new Date(latest.asOf).toLocaleTimeString(lang))
    } catch {
      status.classList.add('error')
      $('updated').textContent = T.failed
      if (!latest) {
        document.body.classList.remove('loading')
        document.body.classList.add('load-error')
      }
    }
  }

  let resizeTimer = 0
  window.addEventListener('resize', () => {
    clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => latest && renderTrend(latest.daily), 150)
  })
  document.body.classList.add('loading')
  load()
  setInterval(load, REFRESH_MS)
})()
