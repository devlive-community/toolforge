use super::*;

#[test]
fn parses_basic_csv_with_header() {
    let out = import_csv(ImportArgs {
        csv: "question,answer,deck,tags\n什么是间隔复习,按固定档位安排下次复习,学习,记忆;方法\n\"带,逗号\",\"引号\"\"转义\"\"\",,\n".to_owned(),
        default_deck: String::new(),
    })
    .unwrap();
    assert_eq!(out.errors.len(), 0);
    assert_eq!(out.cards.len(), 2);
    assert_eq!(out.cards[0].deck, "学习");
    assert_eq!(out.cards[0].tags, vec!["记忆", "方法"]);
    assert_eq!(out.cards[1].question, "带,逗号");
    assert_eq!(out.cards[1].answer, "引号\"转义\"");
    assert_eq!(out.cards[1].deck, "default");
}

#[test]
fn row_errors_collected_not_fatal() {
    let out = import_csv(ImportArgs {
        csv: "question,answer,deck,tags\n,没有问题,学习,\n\"未闭合,学习,\n正常问题,正常答案,,\n"
            .to_owned(),
        default_deck: "d".to_owned(),
    })
    .unwrap();
    assert_eq!(out.cards.len(), 1);
    assert_eq!(out.errors.len(), 2);
    assert_eq!(out.errors[0].line, 2);
    assert_eq!(out.errors[0].rule, "fcs.empty_question");
    assert_eq!(out.errors[1].line, 3);
    assert_eq!(out.errors[1].rule, "fcs.bad_quote");
}

#[test]
fn default_deck_and_empty_lines() {
    let out = import_csv(ImportArgs {
        csv: "\nQ,A,,\n\n".to_owned(),
        default_deck: "默认".to_owned(),
    })
    .unwrap();
    assert_eq!(out.cards.len(), 1);
    assert_eq!(out.cards[0].deck, "默认");
}

#[test]
fn text_length_limit() {
    let long = "x".repeat(5001);
    let out = import_csv(ImportArgs {
        csv: format!("question,answer,deck,tags\n{long},a,,\n"),
        default_deck: String::new(),
    })
    .unwrap();
    assert_eq!(out.errors.len(), 1);
    assert_eq!(out.errors[0].rule, "fcs.text_too_long");
    assert!(out.cards.is_empty());
}

#[test]
fn tags_capped() {
    let out = import_csv(ImportArgs {
        csv: "question,answer,deck,tags\nQ,A,,a;b;c;d;e;f;g;h;i\n".to_owned(),
        default_deck: String::new(),
    })
    .unwrap();
    assert_eq!(out.cards[0].tags.len(), 8);
}
