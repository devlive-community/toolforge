use serde_json::json;

use super::*;

#[test]
fn merges_array_elements() {
    let typed = infer(&json!([
        {"id": 1, "name": "a", "score": 1},
        {"id": 2, "score": 2.5, "tag": null},
        {"id": 3, "name": null, "tag": "x"}
    ]));
    let Shape::Array(element) = typed.shape else {
        panic!()
    };
    let Shape::Object(obj) = element.shape else {
        panic!()
    };
    assert_eq!(obj.samples, 3);
    let field = |k: &str| obj.fields.get(k).unwrap();
    assert_eq!(
        (field("id").typed.shape.clone(), field("id").seen),
        (Shape::Int, 3)
    );
    assert_eq!(
        field("name").typed,
        Typed {
            shape: Shape::Str,
            nullable: true
        }
    );
    assert_eq!(field("name").seen, 2);
    assert_eq!(field("score").typed.shape, Shape::Float);
    assert_eq!(
        field("tag").typed,
        Typed {
            shape: Shape::Str,
            nullable: true
        }
    );
    // 字段顺序保持首次出现的顺序
    assert_eq!(
        obj.fields.keys().collect::<Vec<_>>(),
        vec!["id", "name", "score", "tag"]
    );
}

#[test]
fn mixed_and_empty() {
    assert_eq!(
        infer(&json!([1, "a"])).shape,
        Shape::Array(Box::new(Typed {
            shape: Shape::Any,
            nullable: false
        }))
    );
    assert_eq!(
        infer(&json!([])).shape,
        Shape::Array(Box::new(Typed {
            shape: Shape::Unknown,
            nullable: false
        }))
    );
    assert_eq!(
        infer(&json!(null)),
        Typed {
            shape: Shape::Null,
            nullable: true
        }
    );
}
