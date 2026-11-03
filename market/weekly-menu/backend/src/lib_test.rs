use super::*;
use serde_json::json;

fn recipe() -> Recipe {
    Recipe {
        id: "r1".into(),
        name: "番茄炒蛋".into(),
        base_servings: 2.0,
        ingredients: vec![
            Ingredient {
                name: "西红柿".into(),
                amount: 300.0,
                unit: "g".into(),
            },
            Ingredient {
                name: "鸡蛋".into(),
                amount: 3.0,
                unit: "个".into(),
            },
            Ingredient {
                name: "盐".into(),
                amount: 1.0,
                unit: "适量".into(),
            },
        ],
    }
}

#[test]
fn scales_servings_and_converts_units() {
    let out = scale(ScaleArgs {
        base_servings: 2.0,
        servings: 6.0,
        ingredients: recipe().ingredients,
    })
    .unwrap();
    assert_eq!(out.ingredients[0].amount, 900.0);
    assert_eq!(out.ingredients[0].unit, "g", "below 1000g stays in grams");
    assert_eq!(out.ingredients[1].amount, 9.0);
    assert_eq!(out.ingredients[1].unit, "个");
    let salt = &out.ingredients[2];
    assert!(salt.as_is, "适量 stays as-is");
    assert_eq!(salt.amount, 1.0, "vague amounts are not multiplied");
}

#[test]
fn rejects_bad_servings() {
    let err = scale(ScaleArgs {
        base_servings: 0.0,
        servings: 4.0,
        ingredients: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "menu.bad_servings");
    let err = scale(ScaleArgs {
        base_servings: 2.0,
        servings: -1.0,
        ingredients: vec![],
    })
    .unwrap_err();
    assert_eq!(err.code, "menu.bad_servings");
}

#[test]
fn shopping_merges_same_ingredient_across_meals() {
    let menu = vec![
        MealSlot {
            recipe_id: Some("r1".into()),
            servings: 2.0,
        },
        MealSlot {
            recipe_id: Some("r1".into()),
            servings: 4.0,
        },
        MealSlot {
            recipe_id: None,
            servings: 1.0,
        },
    ];
    let out = shopping(ShoppingArgs {
        menu,
        recipes: vec![recipe()],
    })
    .unwrap();
    assert_eq!(out.meals, 2);
    let tomato = out.items.iter().find(|i| i.name == "西红柿").unwrap();
    assert!((tomato.amount - 900.0).abs() < 0.01, "300×1 + 300×2 = 900g");
    assert_eq!(tomato.unit, "g");
    assert_eq!(tomato.from, vec!["番茄炒蛋".to_string()]);
    assert!(out.items.iter().any(|i| i.name == "盐" && i.as_is));
}

#[test]
fn shopping_rejects_unknown_recipe() {
    let err = shopping(ShoppingArgs {
        menu: vec![MealSlot {
            recipe_id: Some("nope".into()),
            servings: 2.0,
        }],
        recipes: vec![recipe()],
    })
    .unwrap_err();
    assert_eq!(err.code, "menu.unknown_recipe");
}

#[test]
fn dispatches_via_plugin() {
    let out = WeeklyMenu::default()
        .call("scale", json!({"baseServings": 2, "servings": 4, "ingredients": [{"name": "米", "amount": 200, "unit": "g"}]}))
        .unwrap();
    assert_eq!(out["ingredients"][0]["amount"], 400.0);
    assert_eq!(
        WeeklyMenu::default()
            .call("nope", json!({}))
            .unwrap_err()
            .code,
        "plugin.function_not_found"
    );
}

#[test]
fn declared_interfaces_match_backend_args() {
    let checked =
        tf_plugin_api::interface::check_declared_interfaces(&WeeklyMenu::default()).unwrap();
    assert_eq!(checked, 2);
}
