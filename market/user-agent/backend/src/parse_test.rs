use super::*;

fn summary(ua: &str) -> String {
    let p = parse(ua);
    let part = |part: &Option<Part>| {
        part.as_ref().map_or("-".to_owned(), |p| {
            format!("{} {}", p.name, p.version.as_deref().unwrap_or("?"))
        })
    };
    format!(
        "{} | {} | {} | {} {} {} | {}",
        part(&p.browser),
        part(&p.engine),
        part(&p.os),
        p.device.kind,
        p.device.vendor.as_deref().unwrap_or("-"),
        p.device.model.as_deref().unwrap_or("-"),
        p.bot
            .as_ref()
            .map_or("-".to_owned(), |b| format!("{}:{}", b.category, b.name)),
    )
}

#[test]
fn every_rule_compiles_and_ids_are_unique() {
    let t = tables();
    let total =
        t.bots.len() + t.browsers.len() + t.engines.len() + t.systems.len() + t.devices.len();
    let mut ids: Vec<&str> = [
        rules::BOTS,
        rules::BROWSERS,
        rules::ENGINES,
        rules::OPERATING_SYSTEMS,
        rules::DEVICES,
    ]
    .iter()
    .flat_map(|table| table.iter().map(|r| r.id))
    .collect();
    assert_eq!(ids.len(), total);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "duplicate rule ids");
}

#[test]
fn parses_desktop_browsers() {
    let cases = [
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
            "Chrome 129.0.0.0 | Blink 129.0.0.0 | Windows 10/11 | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 Edg/129.0.2792.79",
            "Edge 129.0.2792.79 | Blink 129.0.0.0 | Windows 10/11 | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:131.0) Gecko/20100101 Firefox/131.0",
            "Firefox 131.0 | Gecko 131.0 | macOS 10.15 | desktop Apple Mac | -",
        ),
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
            "Safari 18.0 | WebKit 605.1.15 | macOS 10.15.7 | desktop Apple Mac | -",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36 OPR/114.0.0.0",
            "Opera 114.0.0.0 | Blink 128.0.0.0 | Windows 10/11 | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko",
            "Internet Explorer 11.0 | Trident 7.0 | Windows 7 | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0",
            "Firefox 130.0 | Gecko 130.0 | Ubuntu ? | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
            "Chrome 129.0.0.0 | Blink 129.0.0.0 | ChromeOS 14541.0.0 | desktop - - | -",
        ),
        (
            "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.5359.95 Safari/537.36 QIHU 360SE",
            "360 Browser ? | Blink 108.0.5359.95 | Windows 10/11 | desktop - - | -",
        ),
    ];
    for (ua, expected) in cases {
        assert_eq!(summary(ua), expected, "{ua}");
    }
}

#[test]
fn parses_mobile_and_in_app_browsers() {
    let cases = [
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1",
            "Mobile Safari 17.6 | WebKit 605.1.15 | iOS 17.6.1 | mobile Apple iPhone | -",
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/129.0.6668.69 Mobile/15E148 Safari/604.1",
            "Chrome 129.0.6668.69 | WebKit 605.1.15 | iOS 17.6 | mobile Apple iPhone | -",
        ),
        (
            "Mozilla/5.0 (iPad; CPU OS 16_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Mobile/15E148 Safari/604.1",
            "Mobile Safari 16.6 | WebKit 605.1.15 | iPadOS 16.6 | tablet Apple iPad | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; SM-S9180) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36",
            "Chrome 129.0.0.0 | Blink 129.0.0.0 | Android 14 | mobile Samsung SM-S9180 | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36",
            "Chrome 129.0.0.0 | Blink 129.0.0.0 | Android 10 | mobile - - | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 13; Pixel 7 Build/TQ3A.230805.001; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/129.0.6668.70 Mobile Safari/537.36",
            "Android WebView 129.0.6668.70 | Blink 129.0.6668.70 | Android 13 | mobile Google Pixel 7 | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 12; 22081212C Build/SKQ1.220303.001; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/111.0.5563.116 Mobile Safari/537.36 XWEB/1160065 MMWEBSDK/20231202 MMWEBID/2585 MicroMessenger/8.0.47.2560(0x28002F30) WeChat/arm64 Weixin NetType/WIFI Language/zh_CN ABI/arm64",
            "WeChat 8.0.47.2560 | Blink 111.0.5563.116 | Android 12 | mobile - 22081212C | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 10; HarmonyOS; ALN-AL00; HMSCore 6.13.0.302) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.5735.196 HuaweiBrowser/15.0.4.312 Mobile Safari/537.36",
            "Huawei Browser 15.0.4.312 | Blink 114.0.5735.196 | HarmonyOS ? | mobile Huawei ALN-AL00 | -",
        ),
        (
            "Mozilla/5.0 (Linux; U; Android 12; zh-cn; M2012K11AC Build/SKQ1.211006.001) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/100.0.4896.127 Mobile Safari/537.36 XiaoMi/MiuiBrowser/17.8.150626",
            "MIUI Browser 17.8.150626 | Blink 100.0.4896.127 | Android 12 | mobile - M2012K11AC | -",
        ),
        (
            "Mozilla/5.0 (Linux; Android 13; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
            "Chrome 129.0.0.0 | Blink 129.0.0.0 | Android 13 | tablet Samsung SM-X710 | -",
        ),
        (
            "Mozilla/5.0 (Linux; U; Android 11; zh-CN; V2134A Build/RP1A.200720.012) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/100.0.4896.58 UCBrowser/17.0.6.1310 Mobile Safari/537.36",
            "UC Browser 17.0.6.1310 | Blink 100.0.4896.58 | Android 11 | mobile vivo V2134A | -",
        ),
        (
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
            "iOS WebView ? | WebKit 605.1.15 | iOS 17.5 | mobile Apple iPhone | -",
        ),
        (
            "Mozilla/5.0 (SMART-TV; Linux; Tizen 7.0) AppleWebKit/537.36 (KHTML, like Gecko) 94.0.4606.31/7.0 TV Safari/537.36",
            "- | WebKit 537.36 | Tizen 7.0 | tv - - | -",
        ),
    ];
    for (ua, expected) in cases {
        assert_eq!(summary(ua), expected, "{ua}");
    }
}

#[test]
fn identifies_crawlers_and_tools() {
    let cases = [
        (
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            "search:Googlebot",
        ),
        (
            "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.6668.70 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
            "search:Googlebot",
        ),
        (
            "Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)",
            "search:Baiduspider",
        ),
        (
            "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
            "search:Bingbot",
        ),
        (
            "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.2; +https://openai.com/gptbot)",
            "ai:OpenAI",
        ),
        (
            "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
            "social:Facebook",
        ),
        ("curl/8.7.1", "tool:curl"),
        ("python-requests/2.32.3", "tool:Python Requests"),
        ("Go-http-client/1.1", "tool:Go net/http"),
        (
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/129.0.0.0 Safari/537.36",
            "headless:HeadlessChrome",
        ),
        (
            "Mozilla/5.0 (compatible; AhrefsBot/7.0; +http://ahrefs.com/robot/)",
            "seo:AhrefsBot",
        ),
        (
            "Mozilla/5.0 (compatible; ExampleCrawler/3.1; +https://example.org/bot)",
            "generic:ExampleCrawler",
        ),
        (
            "Mozilla/5.0 (compatible; Acme Monitor; +https://acme.example/info)",
            "generic:Acme Monitor",
        ),
    ];
    for (ua, expected) in cases {
        let p = parse(ua);
        let bot = p.bot.as_ref().expect(ua);
        assert_eq!(format!("{}:{}", bot.category, bot.name), expected, "{ua}");
        assert_eq!(p.device.kind, "bot");
    }
    let bot = parse("Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)")
        .bot
        .unwrap();
    assert_eq!(bot.version.as_deref(), Some("2.1"));
    assert_eq!(bot.hit.rule, "bot.google");
    assert_eq!(bot.hit.matched, "Googlebot/2.1");
}

#[test]
fn ordinary_browsers_and_phone_brands_are_not_bots() {
    for ua in [
        "Mozilla/5.0 (Linux; Android 12; Cubot KingKong 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
    ] {
        assert!(parse(ua).bot.is_none(), "{ua}");
    }
}

#[test]
fn explains_hits_and_adds_interpretation_notes() {
    let p = parse(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
    );
    let browser = p.browser.unwrap();
    assert_eq!(browser.hit.rule, "browser.chrome");
    assert_eq!(browser.hit.matched, "Chrome/129.0.0.0");
    assert_eq!(p.notes, ["frozenPlatform"]);

    let p = parse(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
    );
    assert_eq!(p.notes, ["windows11", "frozenPlatform"]);

    let p = parse(
        "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Mobile Safari/537.36",
    );
    assert_eq!(p.notes, ["reducedModel", "frozenPlatform"]);

    let p = parse(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
    );
    assert_eq!(p.notes, ["macMayBeIpad"]);
}

#[test]
fn detects_user_agents_in_text() {
    assert!(looks_like_user_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36"
    ));
    assert!(looks_like_user_agent("curl/8.7.1"));
    assert!(!looks_like_user_agent("hello world"));
    assert!(!looks_like_user_agent("Mozilla/5.0\nsecond line"));
    assert!(!looks_like_user_agent("the robot spider"));
    let empty = parse("");
    assert!(empty.browser.is_none() && empty.bot.is_none());
    assert_eq!(empty.device.kind, "unknown");
}
