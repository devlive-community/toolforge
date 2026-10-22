use super::*;

fn args(preset: Preset, datacenter_id: u64, worker_id: u64) -> GenerateArgs {
    GenerateArgs {
        preset,
        layout: None,
        datacenter_id,
        worker_id,
    }
}

#[test]
fn decodes_known_twitter_id() {
    // Twitter 公开的推文 ID：2015-01-24 时间附近
    let (unix_ms, datacenter, worker, seq) =
        decode(1_212_161_216_282_000_000, Preset::Twitter.layout().unwrap());
    let expected = 1_288_834_974_657 + (1_212_161_216_282_000_000u64 >> 22) as i64;
    assert_eq!(unix_ms, expected);
    assert_eq!(datacenter, Some((1_212_161_216_282_000_000u64 >> 17) & 31));
    assert_eq!(worker, (1_212_161_216_282_000_000u64 >> 12) & 31);
    assert_eq!(seq, 1_212_161_216_282_000_000u64 & 4095);
}

#[test]
fn decodes_discord_snowflake() {
    // Discord 文档示例：175928847299117063 → 2016-04-30T11:18:25.796Z
    let (unix_ms, datacenter, worker, seq) =
        decode(175_928_847_299_117_063, Preset::Discord.layout().unwrap());
    assert_eq!(unix_ms, 1_462_015_105_796);
    assert_eq!(datacenter, Some(1));
    assert_eq!(worker, 0);
    assert_eq!(seq, 7);
}

#[test]
fn generated_ids_roundtrip_and_increase() {
    let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let now = 1_760_000_000_000;
    let ids = generate_at(&args(Preset::Twitter, 3, 17), 5000, now).unwrap();
    let numbers: Vec<u64> = ids.iter().map(|id| id.parse().unwrap()).collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]));
    let (unix_ms, datacenter, worker, seq) = decode(numbers[0], Preset::Twitter.layout().unwrap());
    assert!(unix_ms >= now);
    assert_eq!(datacenter, Some(3));
    assert_eq!(worker, 17);
    assert!(seq <= 4095);
    // 5000 个超过单个毫秒 4096 个序列号，会借用后续刻度
    let (last_ms, ..) = decode(*numbers.last().unwrap(), Preset::Twitter.layout().unwrap());
    assert!(last_ms > unix_ms);
}

#[test]
fn consecutive_calls_stay_monotonic_for_same_layout() {
    // LAST 是全局状态：两次调用之间不能被并行测试改写，否则时钟回拨分支失效
    let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let now = 1_770_000_000_000;
    let first = generate_at(&args(Preset::Discord, 1, 1), 3, now).unwrap();
    // 时钟回拨到更早时间仍在上次之后
    let second = generate_at(&args(Preset::Discord, 1, 1), 3, now - 10).unwrap();
    let a: u64 = first.last().unwrap().parse().unwrap();
    let b: u64 = second[0].parse().unwrap();
    assert!(b > a);
}

#[test]
fn baidu_uses_second_ticks() {
    let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let now = 1_760_000_000_123;
    let ids = generate_at(&args(Preset::Baidu, 0, 2_000_000), 1, now).unwrap();
    let (unix_ms, datacenter, worker, _) =
        decode(ids[0].parse().unwrap(), Preset::Baidu.layout().unwrap());
    assert_eq!(datacenter, None);
    assert_eq!(worker, 2_000_000);
    assert_eq!(unix_ms % 1000, 0);
    assert!((now - unix_ms) < 1000 || unix_ms > now);
}

#[test]
fn rejects_out_of_range_ids_and_layouts() {
    let err = generate_at(&args(Preset::Twitter, 32, 0), 1, 1_760_000_000_000).unwrap_err();
    assert_eq!(err.code, "uuid.snowflake.invalid_datacenter");
    let err = generate_at(&args(Preset::Leaf, 0, 1024), 1, 1_760_000_000_000).unwrap_err();
    assert_eq!(err.code, "uuid.snowflake.invalid_worker");
    let err = generate_at(&args(Preset::Twitter, 0, 0), 1, 0).unwrap_err();
    assert_eq!(err.code, "uuid.snowflake.before_epoch");
    let bad = GenerateArgs {
        preset: Preset::Custom,
        layout: Some(Layout {
            epoch_ms: 0,
            unit_ms: 1,
            datacenter_bits: 20,
            worker_bits: 20,
            sequence_bits: 20,
        }),
        datacenter_id: 0,
        worker_id: 0,
    };
    assert_eq!(
        generate_at(&bad, 1, 1).unwrap_err().code,
        "uuid.snowflake.invalid_layout"
    );
}

#[test]
fn custom_layout_generates() {
    let _serial = TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let custom = GenerateArgs {
        preset: Preset::Custom,
        layout: Some(Layout {
            epoch_ms: 1_700_000_000_000,
            unit_ms: 1,
            datacenter_bits: 0,
            worker_bits: 8,
            sequence_bits: 10,
        }),
        datacenter_id: 0,
        worker_id: 200,
    };
    let ids = generate_at(&custom, 2, 1_700_000_100_000).unwrap();
    let id: u64 = ids[0].parse().unwrap();
    assert_eq!((id >> 10) & 255, 200);
    assert_eq!((id >> 18) as i64 + 1_700_000_000_000, 1_700_000_100_000);
}

#[test]
fn inspect_lists_all_presets_and_flags_plausible() {
    let decoded = inspect("175928847299117063").unwrap();
    assert_eq!(decoded.len(), PRESETS.len());
    let discord = decoded
        .iter()
        .find(|d| d.preset == Preset::Discord)
        .unwrap();
    assert!(discord.plausible);
    assert_eq!(
        discord.timestamp.as_deref(),
        Some("2016-04-30T11:18:25.796Z")
    );
    assert!(inspect("abc").is_none());
    assert!(inspect("0").is_none());
    assert!(inspect("9223372036854775808").is_none());
}
