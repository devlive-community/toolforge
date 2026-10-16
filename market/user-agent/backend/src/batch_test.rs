use super::*;

const CHROME: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";
const IPHONE: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_6_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Mobile/15E148 Safari/604.1";
const GOOGLEBOT: &str = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

fn args(input: String, mode: Mode) -> Args {
    Args {
        input,
        mode,
        column: String::new(),
        delimiter: ',',
    }
}

#[test]
fn deduplicates_lines_and_counts_distributions() {
    let input = format!("{CHROME}\n{IPHONE}\n\n{CHROME}\n-\n{GOOGLEBOT}\n");
    let out = run(&args(input, Mode::Lines)).unwrap();
    assert_eq!(out.stats.total, 4);
    assert_eq!(out.stats.unique, 3);
    assert_eq!(out.stats.empty, 2);
    assert_eq!(out.stats.bots, 1);
    assert_eq!(out.rows[0].count, 2);
    assert_eq!(out.rows[0].line, 1);
    assert_eq!(
        out.stats.browsers,
        [
            Bucket {
                name: "Chrome".into(),
                count: 2
            },
            Bucket {
                name: "Mobile Safari".into(),
                count: 1
            },
        ]
    );
    assert_eq!(
        out.stats.devices[0],
        Bucket {
            name: "desktop".into(),
            count: 2
        }
    );
    assert_eq!(
        out.stats.bot_categories,
        [Bucket {
            name: "search".into(),
            count: 1
        }]
    );
    assert!(!out.truncated);
}

#[test]
fn extracts_user_agents_from_combined_logs() {
    let log = format!(
        "1.2.3.4 - - [16/Oct/2026:10:00:00 +0800] \"GET / HTTP/1.1\" 200 512 \"-\" \"{CHROME}\"\n\
         5.6.7.8 - - [16/Oct/2026:10:00:01 +0800] \"GET /a?q=\\\"x\\\" HTTP/1.1\" 404 0 \"https://example.org/\" \"{GOOGLEBOT}\" \"10.0.0.1\"\n\
         malformed line\n"
    );
    let out = run(&args(log, Mode::Log)).unwrap();
    assert_eq!(out.skipped, 1);
    let uas: Vec<&str> = out.rows.iter().map(|r| r.ua.as_str()).collect();
    assert_eq!(uas, [CHROME, GOOGLEBOT]);
}

#[test]
fn reads_csv_columns_by_name_or_auto_detection() {
    let csv = format!("ip,User-Agent\n1.1.1.1,\"{CHROME}\"\n2.2.2.2,curl/8.7.1\n");
    let out = run(&args(csv.clone(), Mode::Csv)).unwrap();
    assert_eq!(out.columns, ["ip", "User-Agent"]);
    assert_eq!(out.rows[1].ua, "curl/8.7.1");
    assert_eq!(out.rows[1].line, 3);

    let mut by_index = args(csv.clone(), Mode::Csv);
    by_index.column = "1".into();
    assert_eq!(run(&by_index).unwrap().rows[0].ua, "1.1.1.1");
    let mut missing = args(csv, Mode::Csv);
    missing.column = "agent".into();
    assert_eq!(run(&missing).unwrap_err().code, "ua.column_not_found");
}

#[test]
fn exports_csv_and_json() {
    let input = format!("{CHROME}\ncurl/8.7.1\n");
    let csv = export(ExportArgs {
        args: args(input.clone(), Mode::Lines),
        format: ExportFormat::Csv,
    })
    .unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines[0].starts_with("count,user_agent,browser"));
    assert!(lines[1].starts_with("1,\"Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));
    assert!(lines[1].contains(",Chrome,129.0.0.0,Blink,"));
    assert_eq!(lines[2], "1,curl/8.7.1,,,,,,,bot,,,curl,tool,tool.curl");

    let json = export(ExportArgs {
        args: args(input, Mode::Lines),
        format: ExportFormat::Json,
    })
    .unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["rows"][0]["browser"]["name"], "Chrome");
    assert_eq!(value["stats"]["bots"], 1);
}

#[test]
fn rejects_oversized_input() {
    let big = "a".repeat(MAX_INPUT + 1);
    assert_eq!(
        run(&args(big, Mode::Lines)).unwrap_err().code,
        "ua.too_large"
    );
}
