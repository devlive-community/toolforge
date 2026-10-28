use super::*;

fn args(expenses: &[&str]) -> SplitArgs {
    SplitArgs {
        expenses: expenses.iter().map(|s| s.to_string()).collect(),
        members: vec![],
        settled: vec![],
    }
}

fn transfers(out: &SplitOut) -> Vec<(String, String, String)> {
    out.transfers
        .iter()
        .map(|t| (t.from.clone(), t.to.clone(), t.amount.clone()))
        .collect()
}

#[test]
fn one_payer_split_equally_among_everyone() {
    let out = split(args(&["Ann:90", "Bo:0:Bo|Cy"])).unwrap();
    // 第一笔省略参与者 → 所有人（Ann、Bo、Cy）均摊 90
    assert_eq!(out.total, "90.00");
    let mut t = transfers(&out);
    t.sort();
    assert_eq!(
        t,
        vec![
            ("Bo".into(), "Ann".into(), "30.00".into()),
            ("Cy".into(), "Ann".into(), "30.00".into())
        ]
    );
}

#[test]
fn shares_and_cents_are_conserved() {
    let out = split(args(&["Ann:100:Ann|Bo*2"])).unwrap();
    let share: Vec<&str> = out.balances.iter().map(|b| b.share.as_str()).collect();
    // 100 元按 1:2 分：33.34 + 66.66（余分按成员顺序补，总和不变）
    assert_eq!(share, vec!["33.34", "66.66"]);
    assert_eq!(
        transfers(&out),
        vec![("Bo".into(), "Ann".into(), "66.66".into())]
    );
}

#[test]
fn settled_transfers_reduce_what_is_owed() {
    let mut a = args(&["Ann:60:Ann|Bo"]);
    a.settled = vec!["Bo>Ann:10".into()];
    let out = split(a).unwrap();
    assert_eq!(
        transfers(&out),
        vec![("Bo".into(), "Ann".into(), "20.00".into())]
    );
}

#[test]
fn extra_members_share_unassigned_expenses() {
    let mut a = args(&["Ann:30"]);
    a.members = vec!["Ann".into(), "Bo".into(), "Cy".into()];
    assert_eq!(split(a).unwrap().transfers.len(), 2);
}

#[test]
fn malformed_lines_are_rejected() {
    assert_eq!(split(args(&["Ann"])).unwrap_err().code, "esp.bad_line");
    assert_eq!(split(args(&["Ann:abc"])).unwrap_err().code, "esp.bad_total");
    assert_eq!(
        split(args(&["Ann:10:Bo*x"])).unwrap_err().code,
        "esp.bad_line"
    );
    assert_eq!(split(args(&[])).unwrap_err().code, "esp.no_expenses");
    let mut a = args(&["Ann:10"]);
    a.settled = vec!["Bo-Ann:5".into()];
    assert_eq!(split(a).unwrap_err().code, "esp.bad_line");
}
