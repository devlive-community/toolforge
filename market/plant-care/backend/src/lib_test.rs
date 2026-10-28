use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use tf_plugin_api::LogLevel;
#[derive(Default)]
struct Context {
    cancelled: AtomicBool,
    cancel_on: Option<&'static str>,
}
impl TaskContext for Context {
    fn log(&self, _: LogLevel, _: &str, _: Value) {}
    fn stage(&self, s: &str) {
        if self.cancel_on == Some(s) {
            self.cancelled.store(true, Ordering::Relaxed);
        }
    }
    fn progress(&self, _: u64, _: u64) {}
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
    fn open_file(&self, p: &str) -> PluginResult<Box<dyn Read + Send>> {
        std::fs::File::open(p)
            .map(|f| Box::new(f) as Box<dyn Read + Send>)
            .map_err(|_| error("file"))
    }
    fn file_size(&self, p: &str) -> PluginResult<u64> {
        std::fs::metadata(p)
            .map(|m| m.len())
            .map_err(|_| error("file"))
    }
}
fn change(g: Garden, key: &str, op: Value) -> PluginResult<Garden> {
    ops::apply(
        g,
        serde_json::from_value(op).unwrap(),
        key,
        "2026-10-22",
        &Context::default(),
    )
}
fn garden() -> Garden {
    let g=change(Garden::default(),"plant1",json!({"type":"newPlant","name":"Fern","variety":"Same variety","location":"Window","note":""})).unwrap();
    change(g,"plan1",json!({"type":"newPlan","plant":"plant1","name":"Check soil","first":"2026-10-01","interval":"7"})).unwrap()
}
fn complete(g: Garden, key: &str, date: &str) -> Garden {
    change(
        g,
        key,
        json!({"type":"complete","plant":"plant1","plan":"plan1","date":date,"note":"Actual care"}),
    )
    .unwrap()
}
fn due(g: &Garden) -> String {
    g.due(&g.plants[0].plans[0]).unwrap()
}
#[test]
fn initial_plan_and_late_completion_use_calendar_days_not_accumulated_missed_jobs() {
    let g = garden();
    assert_eq!(due(&g), "2026-10-01");
    let v = view::inspect(
        &g,
        "2026-10-22",
        &view::Query::default(),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(v["page"]["total"], 1);
    assert_eq!(v["counts"]["overdue"], 1);
    let g = complete(g, "done1", "2026-10-22");
    assert_eq!(due(&g), "2026-10-29");
    assert_eq!(g.records.len(), 1);
}
#[test]
fn old_backfill_and_void_recalculate_from_latest_valid_actual_date() {
    let g = complete(garden(), "done1", "2026-10-20");
    let g = complete(g, "done2", "2026-10-05");
    assert_eq!(due(&g), "2026-10-27");
    let g = change(
        g,
        "void",
        json!({"type":"void","record":"done1","voided":true}),
    )
    .unwrap();
    assert_eq!(due(&g), "2026-10-12");
    assert_eq!(g.records.len(), 2);
    let g = change(
        g,
        "restore",
        json!({"type":"void","record":"done1","voided":false}),
    )
    .unwrap();
    assert_eq!(due(&g), "2026-10-27");
}
#[test]
fn repeated_completion_token_is_idempotent_even_after_it_was_voided() {
    let g = complete(garden(), "done1", "2026-10-20");
    let copy = complete(g.clone(), "done1", "2026-10-20");
    assert_eq!(g, copy);
    let g = change(
        g,
        "v",
        json!({"type":"void","record":"done1","voided":true}),
    )
    .unwrap();
    assert_eq!(complete(g.clone(), "done1", "2026-10-20"), g);
    assert!(change(g,"done1",json!({"type":"complete","plant":"plant1","plan":"plan1","date":"2026-10-19","note":"different"})).is_err());
}
#[test]
fn postponement_is_not_completion_and_old_backfill_keeps_it() {
    let g = complete(garden(), "r1", "2026-10-10");
    let g = change(
        g,
        "p",
        json!({"type":"postpone","plant":"plant1","plan":"plan1","date":"2026-10-25"}),
    )
    .unwrap();
    assert_eq!(due(&g), "2026-10-25");
    assert_eq!(g.records.len(), 1);
    let g = complete(g, "old", "2026-10-01");
    assert_eq!(due(&g), "2026-10-25");
    let g = complete(g, "new", "2026-10-22");
    assert_eq!(due(&g), "2026-10-29");
    assert!(g.plants[0].plans[0].postponed.is_none());
}
#[test]
fn interval_edits_preview_without_persisting_then_apply_clears_delay() {
    let plugin = PlantCare::default();
    let g = complete(garden(), "r1", "2026-10-10");
    let g = change(
        g,
        "p",
        json!({"type":"postpone","plant":"plant1","plan":"plan1","date":"2026-10-25"}),
    )
    .unwrap();
    let op = json!({"type":"editPlan","plant":"plant1","plan":"plan1","name":"Check","first":"2026-10-01","interval":"14"});
    let out=plugin.run_task("preview",json!({"state":g,"today":"2026-10-22","requestId":"q","key":"edit","op":op,"query":{"section":"plans"}}),&Context::default()).unwrap();
    assert!(out["state"].is_null());
    assert_eq!(out["view"]["page"]["items"][0]["due"], "2026-10-24");
    assert_eq!(due(&g), "2026-10-25");
    let next = change(g, "e", op).unwrap();
    assert_eq!(due(&next), "2026-10-24");
}
#[test]
fn pause_archive_resume_preserve_every_record_and_original_due_date() {
    let g = complete(garden(), "r1", "2026-10-01");
    let g = change(
        g,
        "p",
        json!({"type":"pause","plant":"plant1","plan":"plan1","paused":true}),
    )
    .unwrap();
    assert!(
        change(
            g.clone(),
            "bad",
            json!({"type":"complete","plant":"plant1","plan":"plan1","date":"2026-10-22","note":""})
        )
        .is_err()
    );
    let v = view::inspect(
        &g,
        "2026-10-22",
        &view::Query::default(),
        &Context::default(),
    )
    .unwrap();
    assert_eq!(v["page"]["total"], 0);
    let g = change(
        g,
        "a",
        json!({"type":"archive","plant":"plant1","archived":true}),
    )
    .unwrap();
    let g = change(
        g,
        "u",
        json!({"type":"pause","plant":"plant1","plan":"plan1","paused":false}),
    )
    .unwrap();
    assert_eq!(
        view::inspect(
            &g,
            "2026-10-22",
            &view::Query::default(),
            &Context::default()
        )
        .unwrap()["page"]["total"],
        0
    );
    let g = change(
        g,
        "a2",
        json!({"type":"archive","plant":"plant1","archived":false}),
    )
    .unwrap();
    assert_eq!(due(&g), "2026-10-08");
    assert_eq!(g.records.len(), 1);
}
#[test]
fn same_variety_plants_and_same_task_names_keep_stable_independent_ids() {
    let g = garden();
    let g=change(g,"plant2",json!({"type":"newPlant","name":"Fern","variety":"Same variety","location":"Door","note":""})).unwrap();
    let g=change(g,"plan2",json!({"type":"newPlan","plant":"plant2","name":"Check soil","first":"2026-10-03","interval":"3"})).unwrap();
    let g = complete(g, "r1", "2026-10-20");
    assert_eq!(g.base(&g.plants[1].plans[0]).unwrap(), "2026-10-03");
    assert_eq!(g.records[0].plant_id, "plant1");
}
#[test]
fn dates_cross_month_year_and_leap_boundaries_without_elapsed_hour_arithmetic() {
    for (start, days, want) in [
        ("2024-02-28", 2, "2024-03-01"),
        ("2026-12-31", 1, "2027-01-01"),
        ("2026-03-07", 2, "2026-03-09"),
    ] {
        assert_eq!(date::format_date(day(start).unwrap() + days), want);
    }
    for date in [
        "2026-02-29",
        "2026-13-01",
        "+026-01-01",
        "2026-1-01",
        "1899-01-01",
    ] {
        assert!(day(date).is_err());
    }
    let mut g = garden();
    g.plants[0].plans[0].first = "9999-12-31".into();
    let g=ops::apply(g,serde_json::from_value(json!({"type":"complete","plant":"plant1","plan":"plan1","date":"9999-12-31","note":""})).unwrap(),"r1","9999-12-31",&Context::default());
    assert!(g.is_err());
}
#[test]
fn future_dates_bad_intervals_and_earlier_deferrals_are_rejected() {
    for i in ["0", "3651", "-1", "1.5", "1e2", ""] {
        assert!(change(garden(),"p2",json!({"type":"newPlan","plant":"plant1","name":"X","first":"2026-10-22","interval":i})).is_err());
    }
    assert!(
        change(
            garden(),
            "r1",
            json!({"type":"complete","plant":"plant1","plan":"plan1","date":"2026-10-23","note":""})
        )
        .is_err()
    );
    assert!(
        change(
            garden(),
            "p",
            json!({"type":"postpone","plant":"plant1","plan":"plan1","date":"2026-10-21"})
        )
        .is_err()
    );
}
#[test]
fn linked_records_and_covers_are_checked_before_state_is_accepted() {
    let mut g = complete(garden(), "r1", "2026-10-20");
    g.records[0].plant_id = "other".into();
    assert!(validate(&g, &Context::default()).is_err());
    let mut g = garden();
    g.plants[0].cover = Some("gone".into());
    assert!(validate(&g, &Context::default()).is_err());
}
#[test]
fn timeline_and_plant_lists_are_sorted_filtered_and_paginated_in_rust() {
    let mut g = garden();
    for i in 0..13 {
        g = complete(g, &format!("r{i}"), &format!("2026-10-{:02}", i + 1));
    }
    let q: view::Query = serde_json::from_value(json!({"section":"timeline","offset":10})).unwrap();
    let v = view::inspect(&g, "2026-10-22", &q, &Context::default()).unwrap();
    assert_eq!(v["page"]["total"], 13);
    assert_eq!(v["page"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(v["page"]["items"][0]["date"], "2026-10-03");
    let q: view::Query = serde_json::from_value(json!({"search":"WINDOW"})).unwrap();
    assert_eq!(
        view::inspect(&g, "2026-10-22", &q, &Context::default()).unwrap()["plants"]["total"],
        1
    );
}
#[test]
fn delete_requires_confirmation_and_never_removes_another_pot() {
    let g = complete(garden(), "r1", "2026-10-20");
    assert!(
        change(
            g.clone(),
            "del",
            json!({"type":"deletePlant","plant":"plant1","confirm":false})
        )
        .is_err()
    );
    let g = change(
        g,
        "plant2",
        json!({"type":"newPlant","name":"Other","variety":"","location":"","note":""}),
    )
    .unwrap();
    let g = change(
        g,
        "del",
        json!({"type":"deletePlant","plant":"plant1","confirm":true}),
    )
    .unwrap();
    assert_eq!(g.plants.len(), 1);
    assert_eq!(g.plants[0].id, "plant2");
    assert!(g.records.is_empty());
}
#[test]
fn task_results_use_camel_case_and_do_not_return_state_until_requested_mutation() {
    let plugin = PlantCare::default();
    let g = garden();
    let out = plugin
        .run_task(
            "inspect",
            json!({"state":g,"today":"2026-10-22","requestId":"q"}),
            &Context::default(),
        )
        .unwrap();
    assert_eq!(out["baseRevision"], 2);
    assert!(out["state"].is_null());
    assert_eq!(out["view"]["page"]["items"][0]["baseDue"], "2026-10-01");
    let ctx = Context {
        cancel_on: Some("view"),
        ..Context::default()
    };
    assert!(plugin.run_task("apply",json!({"state":g,"today":"2026-10-22","requestId":"q","key":"r1","op":{"type":"complete","plant":"plant1","plan":"plan1","date":"2026-10-22","note":""}}),&ctx).is_err());
}
#[test]
fn sqlite_payload_roundtrips_records_plans_pause_and_selection() {
    let g = complete(garden(), "r1", "2026-10-20");
    let bytes = serde_json::to_vec(&g).unwrap();
    let restored: Garden = serde_json::from_slice(&bytes).unwrap();
    validate(&restored, &Context::default()).unwrap();
    assert_eq!(restored, g);
    assert_eq!(due(&restored), "2026-10-27");
}
fn fixture() -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("tf-plant-{}", id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("own-photo.png");
    image::RgbaImage::from_pixel(180, 120, image::Rgba([20, 140, 50, 255]))
        .save(&path)
        .unwrap();
    (dir, path)
}
#[test]
fn imported_photo_survives_source_deletion_and_can_be_compared_from_state() {
    let (dir, path) = fixture();
    let g=change(garden(),"photo1",json!({"type":"addPhoto","plant":"plant1","date":"2026-10-20","note":"New leaf","path":path})).unwrap();
    std::fs::remove_file(&path).unwrap();
    validate(&g, &Context::default()).unwrap();
    assert_eq!(g.plants[0].cover.as_deref(), Some("photo1"));
    assert!(g.photos[0].data.starts_with("data:image/jpeg;base64,"));
    assert!(g.photos[0].bytes <= 49152);
    let g2=change(g.clone(),"photo1",json!({"type":"addPhoto","plant":"plant1","date":"2026-10-20","note":"New leaf","path":path})).unwrap();
    assert_eq!(g, g2);
    let plugin = PlantCare::default();
    let out = plugin
        .run_task(
            "photos",
            json!({"state":g,"today":"2026-10-22","requestId":"q","ids":["photo1"]}),
            &Context::default(),
        )
        .unwrap();
    assert_eq!(out["photos"][0]["note"], "New leaf");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn removing_photo_clears_cover_and_json_backup_retains_the_embedded_image() {
    let (dir, path) = fixture();
    let g = change(
        garden(),
        "photo1",
        json!({"type":"addPhoto","plant":"plant1","date":"2026-10-20","note":"","path":path}),
    )
    .unwrap();
    let plugin = PlantCare::default();
    let backup = dir.join("garden.json");
    plugin
        .run_task(
            "export",
            json!({"state":g,"today":"2026-10-22","requestId":"q","format":"json","path":backup}),
            &Context::default(),
        )
        .unwrap();
    let out = plugin
        .run_task(
            "restore",
            json!({"state":Garden::default(),"today":"2026-10-22","requestId":"r","path":backup}),
            &Context::default(),
        )
        .unwrap();
    assert!(
        out["state"]["photos"][0]["data"]
            .as_str()
            .unwrap()
            .starts_with("data:image/jpeg;base64,")
    );
    assert!(
        change(
            g.clone(),
            "d",
            json!({"type":"removePhoto","photo":"photo1","confirm":false})
        )
        .is_err()
    );
    let g = change(
        g,
        "d",
        json!({"type":"removePhoto","photo":"photo1","confirm":true}),
    )
    .unwrap();
    assert!(g.photos.is_empty());
    assert!(g.plants[0].cover.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn capacity_limits_reject_and_do_not_trim_history() {
    let mut g = garden();
    g.records = (0..MAX_RECORDS)
        .map(|i| Record {
            id: format!("r{i}"),
            plant_id: "plant1".into(),
            plan_id: "plan1".into(),
            date: "2026-10-01".into(),
            note: "".into(),
            voided: false,
        })
        .collect();
    assert!(
        change(
            g.clone(),
            "r-new",
            json!({"type":"complete","plant":"plant1","plan":"plan1","date":"2026-10-22","note":""})
        )
        .is_err()
    );
    assert_eq!(g.records.len(), MAX_RECORDS);
    g.records.iter_mut().for_each(|r| r.note = "x".repeat(1000));
    assert_eq!(
        validate(&g, &Context::default()).unwrap_err().code,
        "plant-care.storage"
    );
}

#[test]
fn jpeg_exif_orientation_is_applied_and_saved_copy_has_no_orientation_dependency() {
    let (dir, _) = fixture();
    let path = dir.join("rotated.jpg");
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut bytes)
        .encode_image(&image::RgbImage::from_pixel(
            180,
            120,
            image::Rgb([30, 130, 40]),
        ))
        .unwrap();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut oriented = vec![0xff, 0xd8, 0xff, 0xe1, 0, 34];
    oriented.extend_from_slice(exif);
    oriented.extend_from_slice(&bytes[2..]);
    std::fs::write(&path, oriented).unwrap();
    let g = change(
        garden(),
        "photo1",
        json!({"type":"addPhoto","plant":"plant1","date":"2026-10-20","note":"","path":path}),
    )
    .unwrap();
    assert_eq!((g.photos[0].width, g.photos[0].height), (120, 180));
    validate(&g, &Context::default()).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn photo_cancel_and_malformed_restores_never_produce_a_candidate_state() {
    let (dir, path) = fixture();
    let plugin = PlantCare::default();
    let ctx = Context {
        cancel_on: Some("readPhoto"),
        ..Context::default()
    };
    assert_eq!(plugin.run_task("apply",json!({"state":garden(),"today":"2026-10-22","requestId":"q","key":"photo1","op":{"type":"addPhoto","plant":"plant1","date":"2026-10-20","note":"","path":path}}),&ctx).unwrap_err().code,"task.cancelled");
    let invalid = dir.join("invalid.json");
    std::fs::write(&invalid, b"{\"schemaVersion\":99}").unwrap();
    assert!(
        plugin
            .run_task(
                "restore",
                json!({"state":garden(),"today":"2026-10-22","requestId":"q","path":invalid}),
                &Context::default()
            )
            .is_err()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_dispatched_function_is_declared_in_manifest() {
    let plugin = PlantCare::default();
    let declared = &plugin.manifest().functions;
    for name in FUNCTIONS {
        assert!(declared.contains_key(name), "{name} 未在 manifest 中声明");
    }
    assert_eq!(declared.len(), FUNCTIONS.len());
}

#[test]
fn declared_interfaces_are_accepted_by_backend() {
    let plugin = PlantCare::default();
    let checked = tf_plugin_api::interface::check_declared_interfaces(&plugin)
        .expect("declared interfaces must match the backend");
    assert!(checked >= 1);
}
