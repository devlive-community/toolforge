use super::*;

fn args(stitches: u32, rules: &[&str]) -> ShapingArgs {
    ShapingArgs {
        stitches,
        rules: rules.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn increases_every_few_rows_add_up() {
    let out = shaping(args(60, &["Sleeve:7:6:8:2"])).unwrap();
    assert_eq!(out.lines.len(), 8);
    assert_eq!(out.lines[0].line, 7);
    assert_eq!(out.lines[1].line, 13);
    assert_eq!(out.last_line, 49);
    assert_eq!(out.final_stitches, 76);
}

#[test]
fn rules_due_on_the_same_line_are_combined() {
    let out = shaping(args(40, &["Left:4:4:3:-1", "Right:4:4:3:-1"])).unwrap();
    assert_eq!(out.lines.len(), 3);
    assert_eq!(out.lines[0].delta, -2);
    assert_eq!(out.lines[0].changes, "Left #1, Right #1");
    assert_eq!(out.final_stitches, 34);
}

#[test]
fn going_below_zero_is_refused() {
    assert_eq!(
        shaping(args(2, &["Cast off:1:1:5:-1"])).unwrap_err().code,
        "knitting.stitches"
    );
}

#[test]
fn malformed_rules_are_rejected() {
    assert_eq!(
        shaping(args(10, &["A:1:0:3:1"])).unwrap_err().code,
        "knitting.reminder"
    );
    assert_eq!(
        shaping(args(10, &["A:1:2:3:0"])).unwrap_err().code,
        "knitting.reminder"
    );
    assert_eq!(
        shaping(args(10, &["A:1:2"])).unwrap_err().code,
        "knitting.reminder"
    );
    assert_eq!(
        shaping(args(10, &["A:x:2:3:1"])).unwrap_err().code,
        "knitting.number"
    );
    assert_eq!(shaping(args(10, &[])).unwrap_err().code, "knitting.limits");
}
