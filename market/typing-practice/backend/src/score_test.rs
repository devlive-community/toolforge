use super::*;

fn snap(typed: &str, added: u32, wrong_added: u32) -> Snapshot {
    Snapshot {
        typed: typed.to_string(),
        added,
        wrong_added,
        pasted: false,
    }
}

#[test]
fn perfect_typing_is_completed_with_full_accuracy() {
    let (s, score) = apply("abc", &Session::default(), &snap("abc", 3, 0), 60_000);
    assert_eq!(score.correct, 3);
    assert_eq!(score.wrong_now, 0);
    assert!(score.completed);
    assert_eq!(score.accuracy, Some(1.0));
    assert_eq!(s.wrong_attempts, 0);
    // 1 分钟 3 个字符：CPM 3，WPM 0.6
    assert_eq!(score.cpm, Some(3.0));
    assert!((score.wpm.unwrap() - 0.6).abs() < 1e-9);
}

#[test]
fn correction_does_not_erase_recorded_wrong_attempts() {
    // 第一次输入 'x' 错误（位置 0）
    let (s1, _) = apply("abc", &Session::default(), &snap("x", 1, 1), 1_000);
    // 退格后正确输入 'a'
    let (s2, score) = apply("abc", &s1, &snap("a", 1, 0), 2_000);
    assert_eq!(s2.attempts, 2);
    assert_eq!(s2.wrong_attempts, 1, "纠正不应抹掉已发生的错误尝试");
    assert_eq!(score.accuracy, Some(0.5));
    assert!(!score.completed);
}

#[test]
fn chinese_multi_char_commit_counts_each_character() {
    // 中文 IME 一次提交两个字，按字符（非字节）计数
    let target = "你好世界";
    let (s, score) = apply(target, &Session::default(), &snap("你好", 2, 0), 4_000);
    assert_eq!(s.attempts, 2);
    assert_eq!(score.correct, 2);
    assert_eq!(score.total, 4);
    assert!((score.progress - 0.5).abs() < 1e-9);
}

#[test]
fn zero_elapsed_time_yields_no_speed() {
    let (_, score) = apply("abc", &Session::default(), &snap("abc", 3, 0), 0);
    assert_eq!(score.cpm, None);
    assert_eq!(score.wpm, None);
}

#[test]
fn no_input_yields_no_accuracy_or_speed() {
    let (s, score) = apply("abc", &Session::default(), &snap("", 0, 0), 0);
    assert_eq!(s.attempts, 0);
    assert_eq!(score.accuracy, None);
    assert_eq!(score.cpm, None);
    assert!(!score.completed);
}

#[test]
fn repeated_input_is_not_double_counted_when_added_is_zero() {
    let (s1, _) = apply("abc", &Session::default(), &snap("a", 1, 0), 1_000);
    // 没有新增字符时（例如重新渲染），累计不变
    let (s2, _) = apply("abc", &s1, &snap("a", 0, 0), 1_500);
    assert_eq!(s2.attempts, 1);
    assert_eq!(s2.wrong_attempts, 0);
}

#[test]
fn paste_marks_session_as_practice_and_is_sticky() {
    let pasted = Snapshot {
        typed: "abc".into(),
        added: 3,
        wrong_added: 0,
        pasted: true,
    };
    let (s1, score) = apply("abc", &Session::default(), &pasted, 1_000);
    assert!(score.pasted);
    // 之后的普通输入不会清掉练习标记
    let (s2, score2) = apply("abc", &s1, &snap("abc", 0, 0), 1_200);
    assert!(s2.pasted);
    assert!(score2.pasted);
}

#[test]
fn empty_target_never_completes() {
    let (_, score) = apply("", &Session::default(), &snap("", 0, 0), 1_000);
    assert_eq!(score.total, 0);
    assert_eq!(score.progress, 0.0);
    assert!(!score.completed);
}

#[test]
fn extra_characters_beyond_target_are_wrong() {
    let (_, score) = apply("ab", &Session::default(), &snap("abc", 3, 1), 1_000);
    assert_eq!(score.correct, 2);
    assert_eq!(score.wrong_now, 1);
    assert!(!score.completed);
}
