//! 内置规则库：按顺序匹配，第一条命中的规则生效。
//!
//! 每条规则都有稳定的 id 与原始正则，解析结果会带上命中的规则与匹配到的片段，便于人工核对。
//! 正则引擎不支持环视，需要排除的情况写在 `unless` 中（命中 pattern 且不命中 unless 才算命中）。
//! 版本取第一个非空捕获组（name 为空的兜底规则：第 1 组是名称、第 2 组是版本）；`fixed` 用于从内核号换算出的固定版本（如 Windows NT 6.1 → 7）。

/// 规则库版本，规则有变化时递增
pub const RULES_VERSION: &str = "2026.10";

#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub id: &'static str,
    pub pattern: &'static str,
    pub unless: Option<&'static str>,
    pub name: &'static str,
    /// 爬虫类别 / 设备类型等附加标签
    pub tag: &'static str,
    pub fixed: Option<&'static str>,
}

const fn r(id: &'static str, pattern: &'static str, name: &'static str) -> Rule {
    Rule {
        id,
        pattern,
        unless: None,
        name,
        tag: "",
        fixed: None,
    }
}

impl Rule {
    const fn unless(mut self, pattern: &'static str) -> Self {
        self.unless = Some(pattern);
        self
    }

    const fn tag(mut self, tag: &'static str) -> Self {
        self.tag = tag;
        self
    }

    const fn fixed(mut self, version: &'static str) -> Self {
        self.fixed = Some(version);
        self
    }
}

/// 爬虫与自动化客户端；tag 为类别：search / social / ai / seo / monitor / tool / headless / generic
pub const BOTS: &[Rule] = &[
    // 搜索引擎
    r("bot.google", r"Googlebot(?:-Image|-Video|-News)?/([\d.]+)", "Googlebot").tag("search"),
    r("bot.google-other", r"(?:GoogleOther|Google-InspectionTool|Storebot-Google|APIs-Google|AdsBot-Google(?:-Mobile)?|Mediapartners-Google|FeedFetcher-Google|Google-Read-Aloud)", "Google").tag("search"),
    r("bot.bing", r"bingbot/([\d.]+)", "Bingbot").tag("search"),
    r("bot.bing-other", r"(?:BingPreview|msnbot|adidxbot)(?:/([\d.]+))?", "Bing").tag("search"),
    r("bot.baidu", r"(?i)Baiduspider(?:-[a-z]+)?(?:/([\d.]+))?", "Baiduspider").tag("search"),
    r("bot.yandex", r"Yandex(?:Bot|Images|Mobile ?Bot|Metrika|Direct|Accessibility ?Bot|RenderResources ?Bot)/([\d.]+)", "YandexBot").tag("search"),
    r("bot.duckduckgo", r"DuckDuck(?:Go-Favicons-)?Bot(?:-Https)?/([\d.]+)", "DuckDuckBot").tag("search"),
    r("bot.sogou", r"(?i)Sogou (?:web|inst|Pic|News|Video|Orion)? ?spider(?:/([\d.]+))?", "Sogou Spider").tag("search"),
    r("bot.360", r"(?:360Spider|HaosouSpider)", "360Spider").tag("search"),
    r("bot.bytedance", r"Bytespider", "Bytespider").tag("search"),
    r("bot.yisou", r"YisouSpider", "YisouSpider").tag("search"),
    r("bot.petal", r"PetalBot", "PetalBot").tag("search"),
    r("bot.apple", r"Applebot(?:-Extended)?/([\d.]+)", "Applebot").tag("search"),
    r("bot.yahoo", r"Yahoo! Slurp", "Yahoo! Slurp").tag("search"),
    r("bot.seznam", r"SeznamBot/([\d.]+)", "SeznamBot").tag("search"),
    r("bot.naver", r"Yeti/([\d.]+)", "Naver Yeti").tag("search"),
    r("bot.coccoc", r"coccocbot(?:-[a-z]+)?/([\d.]+)", "coccocbot").tag("search"),
    r("bot.qwant", r"Qwantify(?:/([\d.]+))?", "Qwantify").tag("search"),
    // 社交与链接预览
    r("bot.facebook", r"(?:facebookexternalhit|facebookcatalog|Facebot|meta-externalfetcher)(?:/([\d.]+))?", "Facebook").tag("social"),
    r("bot.twitter", r"Twitterbot/([\d.]+)", "Twitterbot").tag("social"),
    r("bot.linkedin", r"LinkedInBot/([\d.]+)", "LinkedInBot").tag("social"),
    r("bot.slack", r"Slack(?:bot|-ImgProxy)(?:-LinkExpanding)?(?: |/)?([\d.]+)?", "Slackbot").tag("social"),
    r("bot.discord", r"Discordbot/([\d.]+)", "Discordbot").tag("social"),
    r("bot.telegram", r"TelegramBot", "TelegramBot").tag("social"),
    r("bot.whatsapp", r"^WhatsApp/([\d.]+)", "WhatsApp").tag("social"),
    r("bot.pinterest", r"Pinterest(?:bot)?/([\d.]+)", "Pinterestbot").tag("social"),
    r("bot.reddit", r"redditbot/([\d.]+)", "redditbot").tag("social"),
    r("bot.skype", r"SkypeUriPreview", "Skype").tag("social"),
    r("bot.embedly", r"Embedly(?:/([\d.]+))?", "Embedly").tag("social"),
    // AI 抓取与助手
    r("bot.gpt", r"(?:GPTBot|ChatGPT-User|OAI-SearchBot)(?:/([\d.]+))?", "OpenAI").tag("ai"),
    r("bot.claude", r"(?:ClaudeBot|Claude-User|Claude-SearchBot|anthropic-ai)(?:/([\d.]+))?", "Anthropic").tag("ai"),
    r("bot.perplexity", r"Perplexity(?:Bot|-User)(?:/([\d.]+))?", "Perplexity").tag("ai"),
    r("bot.ccbot", r"CCBot/([\d.]+)", "CCBot").tag("ai"),
    r("bot.amazon", r"Amazonbot/([\d.]+)", "Amazonbot").tag("ai"),
    r("bot.meta-ai", r"meta-externalagent(?:/([\d.]+))?", "Meta").tag("ai"),
    r("bot.cohere", r"cohere-ai", "Cohere").tag("ai"),
    r("bot.diffbot", r"Diffbot(?:/([\d.]+))?", "Diffbot").tag("ai"),
    // SEO 分析
    r("bot.ahrefs", r"AhrefsBot/([\d.]+)", "AhrefsBot").tag("seo"),
    r("bot.semrush", r"SemrushBot(?:-[A-Za-z]+)?(?:/([\d.~a-z]+))?", "SemrushBot").tag("seo"),
    r("bot.majestic", r"MJ12bot/v?([\d.]+)", "MJ12bot").tag("seo"),
    r("bot.dotbot", r"DotBot/([\d.]+)", "DotBot").tag("seo"),
    r("bot.blex", r"BLEXBot/([\d.]+)", "BLEXBot").tag("seo"),
    r("bot.dataforseo", r"DataForSeoBot/([\d.]+)", "DataForSeoBot").tag("seo"),
    r("bot.screaming-frog", r"Screaming Frog SEO Spider/([\d.]+)", "Screaming Frog").tag("seo"),
    r("bot.moz", r"rogerbot(?:/([\d.]+))?", "rogerbot").tag("seo"),
    r("bot.serpstat", r"serpstatbot/([\d.]+)", "serpstatbot").tag("seo"),
    // 监控
    r("bot.uptimerobot", r"UptimeRobot/([\d.]+)", "UptimeRobot").tag("monitor"),
    r("bot.pingdom", r"Pingdom\.com_bot_version_([\d.]+)", "Pingdom").tag("monitor"),
    r("bot.statuscake", r"StatusCake", "StatusCake").tag("monitor"),
    r("bot.site24x7", r"Site24x7", "Site24x7").tag("monitor"),
    r("bot.datadog", r"Datadog(?:Synthetics| Agent)(?:/([\d.]+))?", "Datadog").tag("monitor"),
    r("bot.newrelic", r"NewRelicPinger(?:/([\d.]+))?", "New Relic").tag("monitor"),
    r("bot.lighthouse", r"Chrome-Lighthouse", "Lighthouse").tag("monitor"),
    r("bot.kube-probe", r"kube-probe/([\d.]+)", "kube-probe").tag("monitor"),
    r("bot.elb", r"ELB-HealthChecker/([\d.]+)", "ELB-HealthChecker").tag("monitor"),
    // 无头浏览器
    r("bot.headless-chrome", r"HeadlessChrome/([\d.]+)", "HeadlessChrome").tag("headless"),
    r("bot.phantomjs", r"PhantomJS/([\d.]+)", "PhantomJS").tag("headless"),
    r("bot.electron-headless", r"(?:Puppeteer|Playwright|Selenium)", "Automation").tag("headless"),
    // 命令行与 HTTP 库
    r("tool.curl", r"^curl/([\d.]+)", "curl").tag("tool"),
    r("tool.wget", r"^Wget/([\d.]+)", "Wget").tag("tool"),
    r("tool.httpie", r"^HTTPie/([\d.]+)", "HTTPie").tag("tool"),
    r("tool.python-requests", r"python-requests/([\d.]+)", "Python Requests").tag("tool"),
    r("tool.python-httpx", r"python-httpx/([\d.]+)", "HTTPX").tag("tool"),
    r("tool.aiohttp", r"aiohttp/([\d.]+)", "aiohttp").tag("tool"),
    r("tool.python-urllib", r"Python-urllib/([\d.]+)", "Python urllib").tag("tool"),
    r("tool.scrapy", r"Scrapy/([\d.]+)", "Scrapy").tag("tool"),
    r("tool.go", r"Go-http-client/([\d.]+)", "Go net/http").tag("tool"),
    r("tool.java-httpclient", r"Apache-HttpClient/([\d.]+)", "Apache HttpClient").tag("tool"),
    r("tool.java", r"^Java/([\d._]+)", "Java").tag("tool"),
    r("tool.okhttp", r"^okhttp/([\d.]+)", "OkHttp").tag("tool"),
    r("tool.axios", r"axios/([\d.]+)", "axios").tag("tool"),
    r("tool.node-fetch", r"node-fetch(?:/([\d.]+))?", "node-fetch").tag("tool"),
    r("tool.undici", r"^undici", "undici").tag("tool"),
    r("tool.postman", r"PostmanRuntime/([\d.]+)", "Postman").tag("tool"),
    r("tool.insomnia", r"insomnia/([\d.]+)", "Insomnia").tag("tool"),
    r("tool.libwww-perl", r"libwww-perl/([\d.]+)", "libwww-perl").tag("tool"),
    r("tool.guzzle", r"GuzzleHttp/([\d.]+)", "Guzzle").tag("tool"),
    r("tool.reqwest", r"^reqwest/([\d.]+)", "reqwest").tag("tool"),
    r("tool.dart", r"^Dart/([\d.]+)", "Dart").tag("tool"),
    r("tool.cfnetwork", r"^[^/ ]+/[\d.]+ CFNetwork/([\d.]+) Darwin", "CFNetwork").tag("tool"),
    // 兜底：名称中带 bot / crawler / spider 的客户端；Cubot 是手机品牌，排除
    r("bot.generic", r"(?i)\b([a-z][a-z0-9._-]*(?:bot|crawler|spider|scraper))\b(?:[/ ]v?([\d.]+))?", "")
        .unless(r"(?i)\bcubot\b")
        .tag("generic"),
    r("bot.generic-url", r"\(compatible; ?([^;/)]+)(?:/([\d.]+))?; ?\+?https?://", "").tag("generic"),
];

/// 浏览器（含应用内浏览器）；顺序决定优先级：套壳与应用内浏览器在 Chrome / Safari 之前
pub const BROWSERS: &[Rule] = &[
    // 应用内
    r("browser.wecom", r"wxwork/([\d.]+)", "WeCom"),
    r("browser.wechat", r"MicroMessenger/([\d.]+)", "WeChat"),
    r("browser.qq-app", r"\bQQ/([\d.]+)", "QQ"),
    r("browser.alipay", r"AlipayClient/([\d.]+)", "Alipay"),
    r("browser.dingtalk", r"DingTalk/([\d.]+)", "DingTalk"),
    r("browser.lark", r"(?:Lark|Feishu)/([\d.]+)", "Lark"),
    r("browser.weibo", r"__weibo__([\d.]+)", "Weibo"),
    r("browser.douyin", r"aweme(?:_lite)?_([\d.]+)", "Douyin"),
    r("browser.baidu-app", r"baiduboxapp/([\d.]+)", "Baidu App"),
    r("browser.facebook-app", r"FBAV/([\d.]+)", "Facebook App"),
    r("browser.instagram", r"Instagram ([\d.]+)", "Instagram"),
    r("browser.line", r"\bLine/([\d.]+)", "LINE"),
    // 国内与厂商浏览器
    r("browser.qq", r"M?QQBrowser/([\d.]+)", "QQ Browser"),
    r("browser.uc", r"UC ?Browser/([\d.]+)", "UC Browser"),
    r("browser.quark", r"Quark/([\d.]+)", "Quark"),
    r(
        "browser.360",
        r"(?:QihooBrowser|QHBrowser|360SE|360EE)(?:/([\d.]+))?",
        "360 Browser",
    ),
    r(
        "browser.sogou",
        r"(?:MetaSr|SogouMobileBrowser/([\d.]+)|SE 2\.X)",
        "Sogou Browser",
    ),
    r("browser.2345", r"2345Explorer/([\d.]+)", "2345 Explorer"),
    r("browser.liebao", r"LBBROWSER", "Liebao"),
    r("browser.maxthon", r"Maxthon/([\d.]+)", "Maxthon"),
    r(
        "browser.huawei",
        r"HuaweiBrowser/([\d.]+)",
        "Huawei Browser",
    ),
    r(
        "browser.miui",
        r"(?:MiuiBrowser|XiaoMi/MiuiBrowser)/([\d.]+)",
        "MIUI Browser",
    ),
    r(
        "browser.heytap",
        r"HeyTapBrowser/([\d.]+)",
        "HeyTap Browser",
    ),
    r("browser.oppo", r"OppoBrowser/([\d.]+)", "OPPO Browser"),
    r("browser.vivo", r"VivoBrowser/([\d.]+)", "vivo Browser"),
    r(
        "browser.samsung",
        r"SamsungBrowser/([\d.]+)",
        "Samsung Internet",
    ),
    r("browser.yandex", r"YaBrowser/([\d.]+)", "Yandex Browser"),
    r("browser.vivaldi", r"Vivaldi/([\d.]+)", "Vivaldi"),
    r("browser.brave", r"Brave(?: Chrome)?/([\d.]+)", "Brave"),
    r("browser.duckduckgo", r"DuckDuckGo/([\d.]+)", "DuckDuckGo"),
    r("browser.whale", r"Whale/([\d.]+)", "Whale"),
    r("browser.silk", r"Silk/([\d.]+)", "Silk"),
    // Opera
    r("browser.opera-mini", r"Opera Mini/([\d.]+)", "Opera Mini"),
    r("browser.opera-gx", r"OPRGX/([\d.]+)", "Opera GX"),
    r("browser.opera", r"(?:OPR|OPiOS|OPT)/([\d.]+)", "Opera"),
    r(
        "browser.opera-presto",
        r"Opera[/ ].*Version/([\d.]+)",
        "Opera",
    ),
    // Edge
    r("browser.edge", r"(?:Edg|EdgA|EdgiOS)/([\d.]+)", "Edge"),
    r("browser.edge-legacy", r"Edge/([\d.]+)", "Edge Legacy"),
    // Firefox 系
    r("browser.firefox-ios", r"FxiOS/([\d.]+)", "Firefox"),
    r("browser.seamonkey", r"SeaMonkey/([\d.]+)", "SeaMonkey"),
    r("browser.waterfox", r"Waterfox/([\d.]+)", "Waterfox"),
    r("browser.firefox-focus", r"Focus/([\d.]+)", "Firefox Focus"),
    r("browser.firefox", r"Firefox/([\d.]+)", "Firefox"),
    // Chrome 系
    r("browser.chrome-ios", r"CriOS/([\d.]+)", "Chrome"),
    r(
        "browser.android-webview",
        r"; wv\).*Chrome/([\d.]+)",
        "Android WebView",
    ),
    r("browser.chromium", r"Chromium/([\d.]+)", "Chromium"),
    r("browser.chrome", r"Chrome/([\d.]+)", "Chrome"),
    // IE
    r("browser.ie", r"MSIE ([\d.]+)", "Internet Explorer"),
    r(
        "browser.ie11",
        r"Trident/[\d.]+.*rv:([\d.]+)",
        "Internet Explorer",
    ),
    // Safari 与系统 WebView
    r(
        "browser.android-browser",
        r"Android.*Version/([\d.]+).*Safari",
        "Android Browser",
    ),
    r(
        "browser.mobile-safari",
        r"Version/([\d.]+).*Mobile/\S+ Safari",
        "Mobile Safari",
    ),
    r("browser.safari", r"Version/([\d.]+).*Safari/", "Safari"),
    r(
        "browser.ios-webview",
        r"(?:iPhone|iPad|iPod).*AppleWebKit/[\d.]+ \(KHTML, like Gecko\)(?: Mobile/\S+)?$",
        "iOS WebView",
    ),
];

/// 渲染引擎
pub const ENGINES: &[Rule] = &[
    r("engine.edgehtml", r"Edge/([\d.]+)", "EdgeHTML"),
    // iOS 上所有浏览器都使用系统 WebKit
    r(
        "engine.webkit-ios",
        r"(?:iPhone|iPad|iPod).*AppleWebKit/([\d.]+)",
        "WebKit",
    ),
    r(
        "engine.blink",
        r"AppleWebKit/[\d.]+.*Chrome/([\d.]+)",
        "Blink",
    ),
    r("engine.presto", r"Presto/([\d.]+)", "Presto"),
    r("engine.trident", r"Trident/([\d.]+)", "Trident"),
    r("engine.gecko", r"rv:([\d.]+)\) Gecko/", "Gecko"),
    r("engine.webkit", r"AppleWebKit/([\d.]+)", "WebKit"),
];

/// 操作系统
pub const OPERATING_SYSTEMS: &[Rule] = &[
    r(
        "os.windows-phone",
        r"Windows Phone(?: OS)? ([\d.]+)",
        "Windows Phone",
    ),
    r("os.windows-10", r"Windows NT 10\.0", "Windows").fixed("10/11"),
    r("os.windows-8.1", r"Windows NT 6\.3", "Windows").fixed("8.1"),
    r("os.windows-8", r"Windows NT 6\.2", "Windows").fixed("8"),
    r("os.windows-7", r"Windows NT 6\.1", "Windows").fixed("7"),
    r("os.windows-vista", r"Windows NT 6\.0", "Windows").fixed("Vista"),
    r("os.windows-xp", r"Windows NT 5\.[12]", "Windows").fixed("XP"),
    r("os.windows", r"Windows(?: NT ([\d.]+))?", "Windows"),
    r(
        "os.harmony",
        r"(?:HarmonyOS|OpenHarmony)(?:[ /;]+([\d.]+))?",
        "HarmonyOS",
    ),
    r("os.ipados", r"iPad.*? OS (\d+[_\d]*)", "iPadOS"),
    r(
        "os.ios",
        r"(?:iPhone|iPod|CPU)(?: iPhone)? OS (\d+[_\d]*)",
        "iOS",
    ),
    r("os.ios-cfnetwork", r"CFNetwork/[\d.]+ Darwin", "iOS"),
    r("os.kaios", r"KAIOS/([\d.]+)", "KaiOS"),
    r("os.android", r"Android(?:[ /]([\d.]+))?", "Android"),
    r("os.chromeos", r"CrOS \S+ ([\d.]+)", "ChromeOS"),
    r("os.macos", r"Mac OS X ?(\d+[_.\d]*)?", "macOS"),
    r("os.tizen", r"Tizen ?([\d.]+)?", "Tizen"),
    r("os.webos", r"(?:Web0S|webOS)(?:/([\d.]+))?", "webOS"),
    r(
        "os.playstation",
        r"PlayStation (?:4|5|Vita|Portable)(?: ([\d.]+))?",
        "PlayStation",
    ),
    r("os.ubuntu", r"Ubuntu(?:/([\d.]+))?", "Ubuntu"),
    r("os.fedora", r"Fedora(?:/([\d.-]+))?", "Fedora"),
    r("os.debian", r"Debian", "Debian"),
    r("os.freebsd", r"FreeBSD", "FreeBSD"),
    r("os.openbsd", r"OpenBSD", "OpenBSD"),
    r("os.linux", r"Linux", "Linux"),
];

/// 设备类型；tag 即类型：tv / console / tablet / mobile / wearable / desktop
pub const DEVICES: &[Rule] = &[
    r("device.tv", r"(?i)(?:SmartTV|SMART-TV|Tizen.*TV|Web0S|webOS.*TV|AppleTV|GoogleTV|Android TV|BRAVIA|HbbTV|CrKey|AFT[A-Z]|Roku)", "").tag("tv"),
    r("device.console", r"(?:PlayStation|Xbox|Nintendo)", "").tag("console"),
    r("device.watch", r"(?i)(?:Watch OS|watchOS|; ?wv\) .*Watch|Wear OS)", "").tag("wearable"),
    r("device.ipad", r"iPad", "").tag("tablet"),
    r("device.kindle", r"(?:Kindle|Silk/)", "").tag("tablet"),
    r("device.tablet", r"(?i)(?:Tablet|; ?Tab[ ;)]|MatePad|MediaPad|Xiaomi Pad|; ?SM-[TX]\d)", "").tag("tablet"),
    // Android 平板的 UA 不含 Mobile
    r("device.android-tablet", r"Android", "")
        .unless(r"(?i)Mobile|Phone")
        .tag("tablet"),
    r("device.mobile", r"(?i)(?:iPhone|iPod|Mobile|Windows Phone|Phone|BlackBerry|KAIOS|Opera Mini)", "").tag("mobile"),
    r("device.desktop", r"(?:Windows NT|Macintosh|X11|CrOS|Linux x86_64)", "").tag("desktop"),
];

/// Android 型号前缀到品牌
pub const VENDORS: &[(&str, &str)] = &[
    ("SM-", "Samsung"),
    ("GT-", "Samsung"),
    ("Galaxy", "Samsung"),
    ("Pixel", "Google"),
    ("Nexus", "Google"),
    ("Redmi", "Xiaomi"),
    ("POCO", "Xiaomi"),
    ("Mi ", "Xiaomi"),
    ("MI ", "Xiaomi"),
    ("Xiaomi", "Xiaomi"),
    ("HUAWEI", "Huawei"),
    ("HONOR", "Honor"),
    ("OPPO", "OPPO"),
    ("CPH", "OPPO"),
    ("PCLM", "OPPO"),
    ("vivo", "vivo"),
    ("V2", "vivo"),
    ("ONEPLUS", "OnePlus"),
    ("OnePlus", "OnePlus"),
    ("RMX", "realme"),
    ("realme", "realme"),
    ("moto", "Motorola"),
    ("Moto", "Motorola"),
    ("Nokia", "Nokia"),
    ("LM-", "LG"),
    ("LG-", "LG"),
    ("SO-", "Sony"),
    ("Xperia", "Sony"),
    ("Lenovo", "Lenovo"),
    ("ZTE", "ZTE"),
    ("Infinix", "Infinix"),
    ("TECNO", "Tecno"),
    ("itel", "itel"),
    ("Meizu", "Meizu"),
    ("MEIZU", "Meizu"),
    ("KFT", "Amazon"),
    ("KF", "Amazon"),
];
