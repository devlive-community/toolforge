use super::*;

fn run(input: &str, configure: impl FnOnce(&mut Args)) -> Output {
    let mut args = Args {
        input: input.into(),
        ..Args::default()
    };
    configure(&mut args);
    process(args)
}

#[test]
fn trims_removes_empty_and_dedupes() {
    let out = run("  b \n\na\nB\n  \nb", |a| {
        a.trim = true;
        a.remove_empty = true;
        a.dedupe = true;
    });
    assert_eq!(out.output, "b\na\nB");
    assert_eq!(
        (out.before, out.after, out.duplicates, out.empty),
        (6, 3, 1, 2)
    );
}

#[test]
fn case_insensitive_dedupe_and_sort() {
    let out = run("b\nA\na\nB", |a| {
        a.dedupe = true;
        a.ignore_case = true;
        a.sort = Sort::Asc;
    });
    assert_eq!(out.output, "A\nb");
}

#[test]
fn natural_and_length_sorting() {
    assert_eq!(
        run("file10\nfile2\nfile1", |a| a.sort = Sort::Natural).output,
        "file1\nfile2\nfile10"
    );
    assert_eq!(
        run("ccc\na\nbb", |a| a.sort = Sort::Length).output,
        "a\nbb\nccc"
    );
    assert_eq!(run("a\nc\nb", |a| a.sort = Sort::Desc).output, "c\nb\na");
    assert_eq!(natural_cmp("v01", "v1"), Ordering::Greater);
}

#[test]
fn numbering_prefix_and_reverse() {
    let input = (1..=10)
        .map(|i| format!("x{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let out = run(&input, |a| {
        a.reverse = true;
        a.number = true;
        a.prefix = "- ".into();
        a.suffix = ";".into();
    });
    assert!(out.output.starts_with(" 1. - x10;\n"));
    assert!(out.output.ends_with("10. - x1;"));
}

#[test]
fn shuffle_keeps_every_line() {
    let input = (0..50)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let out = run(&input, |a| a.shuffle = true);
    let mut shuffled: Vec<&str> = out.output.split('\n').collect();
    shuffled.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(shuffled.join("\n"), input);
}
