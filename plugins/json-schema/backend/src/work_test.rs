use serde_json::{Value, json};

use super::*;

fn passes(schema: &Value, instance: &Value) -> usize {
    check(schema, instance, Draft::Draft202012).unwrap()
}

fn rejects_work(schema: &Value, instance: &Value) {
    assert_eq!(
        check(schema, instance, Draft::Draft202012)
            .unwrap_err()
            .code,
        "schema.work_budget"
    );
}

#[test]
fn ordinary_properties_arrays_and_shared_definitions_fit() {
    let schema = json!({
        "$defs":{"id":{"type":"integer","minimum":0}},
        "type":"object",
        "properties":{
            "id":{"$ref":"#/$defs/id"},
            "ids":{"type":"array","items":{"$ref":"#/$defs/id"}}
        }
    });
    let cost = passes(&schema, &json!({"id":1,"ids":[2,3,4]}));
    assert!(cost > 10 && cost < 100);
}

#[test]
fn reference_dag_charges_cached_cost_every_time() {
    let mut definitions = serde_json::Map::new();
    definitions.insert("n0".into(), json!({"type":"integer"}));
    for i in 1..20 {
        let reference = format!("#/$defs/n{}", i - 1);
        definitions.insert(
            format!("n{i}"),
            json!({"allOf":[{"$ref":reference},{"$ref":reference}]}),
        );
    }
    let schema = json!({"$defs":definitions,"$ref":"#/$defs/n19"});
    rejects_work(&schema, &json!(1));
}

#[test]
fn repeated_progressive_recursion_is_exponential_even_with_few_schema_nodes() {
    let schema = json!({
        "$defs":{"node":{"allOf":[
            {"properties":{"next":{"$ref":"#/$defs/node"}}},
            {"properties":{"next":{"$ref":"#/$defs/node"}}}
        ]}},
        "$ref":"#/$defs/node"
    });
    let mut instance = json!({});
    for _ in 0..20 {
        instance = json!({"next":instance});
    }
    rejects_work(&schema, &instance);
}

#[test]
fn no_progress_scc_costs_are_not_reused_outside_their_active_context() {
    let mut roots = vec![json!({"$ref":"#/$defs/a"})];
    roots.extend(vec![json!({"$ref":"#/$defs/b"}); 20]);
    let schema = json!({
        "$defs":{
            "a":{"allOf":vec![json!({"$ref":"#/$defs/b"});100]},
            "b":{"allOf":vec![json!({"$ref":"#/$defs/a"});100]}
        },
        "allOf":roots
    });
    rejects_work(&schema, &json!({}));
}

#[test]
fn linear_recursive_tree_and_no_progress_self_reference_terminate() {
    let schema = json!({
        "$defs":{"node":{"type":"object","properties":{
            "value":{"type":"integer"},
            "next":{"$ref":"#/$defs/node"}
        }}},
        "$ref":"#/$defs/node"
    });
    let mut instance = json!({"value":0});
    for i in 1..20 {
        instance = json!({"value":i,"next":instance});
    }
    assert!(passes(&schema, &instance) < 500);
    assert!(passes(&json!({"$ref":"#"}), &json!({})) < 10);
    assert!(passes(&json!({"allOf":[{"$ref":"#"},{"$ref":"#"}]}), &json!({})) < 20);
}

#[test]
fn many_errors_are_rejected_before_the_eager_engine_allocates_them() {
    let schema = json!({"allOf":vec![json!({"items":false}); 250]});
    rejects_work(&schema, &json!(vec![0; 5_000]));
}

#[test]
fn referenced_annotation_schema_is_checked_but_unreferenced_data_is_ignored() {
    let payload = json!({"allOf":vec![json!(false);257]});
    let ignored = json!({"examples":[{"$ref":"file:///data"}],"payload":payload});
    passes(&ignored, &json!({}));
    let referenced = json!({"payload":payload,"$ref":"#/payload"});
    let error = check(&referenced, &json!({}), Draft::Draft202012).unwrap_err();
    assert_eq!(error.code, "schema.reference_target");
    assert_eq!(error.params["path"], "/payload/allOf");
}

#[test]
fn pointers_decode_percent_and_tilde_escapes() {
    let schema = json!({"$defs":{"a/b~ x":{"type":"integer"}},"$ref":"#/$defs/a~1b~0%20x"});
    passes(&schema, &json!(1));
    for reference in ["#/$defs/a~2b", "#/$defs/%FF", "#/$defs/%2", "#missing"] {
        let error = check(&json!({"$ref":reference}), &json!(1), Draft::Draft202012).unwrap_err();
        assert_eq!(error.code, "schema.reference_target");
    }
}

#[test]
fn pointers_and_anchors_use_the_current_resource_scope() {
    let schema = json!({
        "$id":"https://example.test/root",
        "$defs":{
            "x":{"type":"boolean"},
            "inner":{
                "$id":"https://example.test/inner",
                "$defs":{"x":{"$anchor":"number","type":"integer"}},
                "allOf":[{"$ref":"#/$defs/x"},{"$ref":"#number"}]
            }
        },
        "$ref":"#/$defs/inner"
    });
    passes(&schema, &json!(1));
    let relative = json!({"$defs":{"inner":{"$id":"child","$ref":"#"}},"$ref":"#/$defs/inner"});
    assert_eq!(
        check(&relative, &json!(1), Draft::Draft202012)
            .unwrap_err()
            .code,
        "schema.reference_target"
    );
}

#[test]
fn supports_draft7_fragment_ids_as_anchors() {
    let schema =
        json!({"definitions":{"integer":{"$id":"#integer","type":"integer"}},"$ref":"#integer"});
    check(&schema, &json!(1), Draft::Draft7).unwrap();
}

#[test]
fn dynamic_anchors_and_property_name_strings_are_accounted_conservatively() {
    let schema = json!({
        "$dynamicAnchor":"node",
        "properties":{"next":{"$dynamicRef":"#node"}},
        "propertyNames":{"allOf":[{"type":"string"},{"$ref":"#/$defs/key"}]},
        "$defs":{"key":{"type":"string","minLength":1}}
    });
    assert!(passes(&schema, &json!({"a":1,"next":{"b":2}})) < 150);
}

#[test]
fn estimated_error_path_storage_has_its_own_budget() {
    let mut instance = json!(0);
    let mut schema = json!({"allOf":vec![json!(false);128]});
    for _ in 0..4 {
        instance = json!({"x".repeat(500):instance});
        schema = json!({"patternProperties":{".":schema}});
    }
    // One property alone is fine; duplicating its evaluation in a DAG is not.
    passes(&schema, &instance);
    let expanded =
        json!({"$defs":{"branch":schema},"allOf":vec![json!({"$ref":"#/$defs/branch"});128]});
    rejects_work(&expanded, &instance);
}

#[test]
fn large_enum_and_required_lists_pay_for_each_evaluation() {
    let schema = json!({"items":{"enum":(0..2_000).collect::<Vec<_>>()}});
    rejects_work(&schema, &json!(vec![10_000; 2_000]));
    let schema =
        json!({"items":{"required":(0..2_000).map(|i| format!("k{i}")).collect::<Vec<_>>()}});
    rejects_work(&schema, &json!(vec![json!({}); 2_000]));
}

#[test]
fn unique_items_uses_the_engines_linear_hashing_path_for_large_arrays() {
    let cost = passes(
        &json!({"uniqueItems":true}),
        &json!((0..1_000).collect::<Vec<_>>()),
    );
    assert!(cost > 1_000 && cost < 2_000);
}

#[test]
fn references_crossing_unregistered_resource_ids_fail_closed() {
    let schema = json!({"payload":{"$id":"https://example.test/hidden","inner":{"$ref":"#"}},"$ref":"#/payload/inner"});
    assert_eq!(
        check(&schema, &json!(null), Draft::Draft202012)
            .unwrap_err()
            .code,
        "schema.reference_target"
    );
}

#[test]
fn long_ref_chains_hit_a_stack_budget_even_if_total_work_is_small() {
    let mut definitions = serde_json::Map::new();
    definitions.insert("n0".into(), json!(true));
    for i in 1..300 {
        definitions.insert(format!("n{i}"), json!({"$ref":format!("#/$defs/n{}", i-1)}));
    }
    rejects_work(
        &json!({"$defs":definitions,"$ref":"#/$defs/n299"}),
        &json!(null),
    );
}
