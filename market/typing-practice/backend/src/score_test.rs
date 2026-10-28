use super::*;

fn snap(prev: &str, typed: &str) -> Snapshot {
    Snapshot {
        prev: prev.to_string(),
        typed: typed.to_string(),
        pasted: false,
    }
}

#[test]
fn perfect_typing_is_completed_with_full_accuracy() {
    let (s, score) = apply("abc", &Session::default(), &snap("", "abc"), 60_000);
    assert_eq!(score.correct, 3);
    assert_eq!(score.wrong_now, 0);
    assert!(score.completed);
    assert!(score.passed);
    assert_eq!(score.accuracy, Some(1.0));
    assert_eq!(s.wrong_attempts, 0);
    // 1 分钟 3 个字符：CPM 3，WPM 0.6
    assert_eq!(score.cpm, Some(3.0));
    assert!((score.wpm.unwrap() - 0.6).abs() < 1e-9);
}

#[test]
fn correction_does_not_erase_recorded_wrong_attempts() {
    let (s1, first) = apply("abc", &Session::default(), &snap("", "x"), 1_000);
    assert_eq!(first.missed, vec!["a".to_string()]);
    // 退格
    let (s2, _) = apply("abc", &s1, &snap("x", ""), 1_500);
    // 正确输入 'a'
    let (s3, score) = apply("abc", &s2, &snap("", "a"), 2_000);
    assert_eq!(s3.attempts, 2);
    assert_eq!(s3.wrong_attempts, 1, "纠正不应抹掉已发生的错误尝试");
    assert_eq!(score.accuracy, Some(0.5));
    assert!(!score.completed);
}

#[test]
fn consecutive_backspaces_add_no_attempts() {
    let (s1, _) = apply("abcd", &Session::default(), &snap("", "abxy"), 1_000);
    let (s2, _) = apply("abcd", &s1, &snap("abxy", "abx"), 1_100);
    let (s3, score) = apply("abcd", &s2, &snap("abx", "ab"), 1_200);
    assert_eq!(s3.attempts, 4);
    assert_eq!(s3.wrong_attempts, 2);
    assert_eq!(score.wrong_now, 0);
    assert_eq!(score.marks, vec![true, true]);
}

#[test]
fn chinese_multi_char_commit_counts_each_character() {
    // 中文 IME 一次提交两个字，按可见字符（非字节）计数
    let (s, score) = apply("你好世界", &Session::default(), &snap("", "你好"), 4_000);
    assert_eq!(s.attempts, 2);
    assert_eq!(score.correct, 2);
    assert_eq!(score.total, 4);
    assert!((score.progress - 0.5).abs() < 1e-9);
}

#[test]
fn emoji_and_combining_marks_count_as_one_character() {
    // 家庭 emoji（ZWJ 序列）与 e + 组合重音各算一个可见字符
    let target = "👨‍👩‍👧e\u{301}!";
    let (s, score) = apply(target, &Session::default(), &snap("", "👨‍👩‍👦e\u{301}!"), 1_000);
    assert_eq!(score.total, 3);
    assert_eq!(s.attempts, 3);
    assert_eq!(s.wrong_attempts, 1, "错误的 emoji 只算一次错误");
    assert_eq!(score.marks, vec![false, true, true]);
    assert_eq!(score.missed, vec!["👨‍👩‍👧".to_string()]);
}

#[test]
fn replacing_a_selection_counts_the_new_characters_as_attempts() {
    let (s1, _) = apply("abcd", &Session::default(), &snap("", "abxd"), 1_000);
    // 选中 x 替换为 c：公共前缀为 "ab"，之后两个字符重新提交
    let (s2, score) = apply("abcd", &s1, &snap("abxd", "abcd"), 2_000);
    assert_eq!(s2.attempts, 6);
    assert_eq!(s2.wrong_attempts, 1);
    assert!(score.completed);
}

#[test]
fn typing_past_the_end_is_wrong_and_blocks_completion() {
    let (s, score) = apply("ab", &Session::default(), &snap("", "abc"), 1_000);
    assert_eq!(s.wrong_attempts, 1);
    assert_eq!(score.wrong_now, 1);
    assert!(!score.completed);
    assert!(score.missed.is_empty(), "超出末尾没有对应的目标字符");
}

#[test]
fn newlines_are_normalized_but_punctuation_is_not() {
    let (_, score) = apply("a\r\nb", &Session::default(), &snap("", "a\nb"), 1_000);
    assert!(score.completed);
    // 全角逗号与半角逗号不等同
    let (_, score) = apply("你，好", &Session::default(), &snap("", "你,好"), 1_000);
    assert_eq!(score.marks, vec![true, false, true]);
}

#[test]
fn zero_elapsed_time_yields_no_speed() {
    let (_, score) = apply("abc", &Session::default(), &snap("", "abc"), 0);
    assert_eq!(score.cpm, None);
    assert_eq!(score.wpm, None);
}

#[test]
fn no_input_yields_no_accuracy_or_speed() {
    let (s, score) = apply("abc", &Session::default(), &snap("", ""), 0);
    assert_eq!(s.attempts, 0);
    assert_eq!(score.accuracy, None);
    assert_eq!(score.cpm, None);
    assert!(!score.completed);
}

#[test]
fn repeated_identical_submission_is_not_double_counted() {
    let (s1, _) = apply("abc", &Session::default(), &snap("", "a"), 1_000);
    // 同一文本再次上报（例如输入法结束事件与 input 事件各报一次）
    let (s2, _) = apply("abc", &s1, &snap("a", "a"), 1_500);
    assert_eq!(s2.attempts, 1);
    assert_eq!(s2.wrong_attempts, 0);
}

#[test]
fn low_accuracy_completion_does_not_pass() {
    let (s1, _) = apply("ab", &Session::default(), &snap("", "xy"), 1_000);
    let (s2, _) = apply("ab", &s1, &snap("xy", ""), 1_100);
    let (_, score) = apply("ab", &s2, &snap("", "ab"), 2_000);
    assert!(score.completed);
    assert_eq!(score.accuracy, Some(0.5));
    assert!(!score.passed);
}

#[test]
fn paste_marks_session_as_practice_and_is_sticky() {
    let pasted = Snapshot {
        prev: String::new(),
        typed: "ab".into(),
        pasted: true,
    };
    let (s1, score) = apply("abc", &Session::default(), &pasted, 1_000);
    assert!(score.pasted);
    let (s2, score) = apply("abc", &s1, &snap("ab", "abc"), 2_000);
    assert!(s2.pasted);
    assert!(score.pasted);
}

#[test]
fn empty_target_never_completes() {
    let (_, score) = apply("", &Session::default(), &snap("", ""), 1_000);
    assert_eq!(score.total, 0);
    assert_eq!(score.progress, 0.0);
    assert!(!score.completed);
}
