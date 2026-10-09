use super::analysis::steps;
use super::show::open_deck;
use super::*;
use crate::show::{self, Transition};

fn transition(worker: &mut Worker, doc: &Doc) -> Transition {
    let read = worker.values(doc, show::TRANSITION).expect("reads");
    show::transition(&read).expect("parses")
}

fn effects(worker: &mut Worker, doc: &Doc) -> Vec<show::Effect> {
    let read = worker.values(doc, show::ANIMATIONS).expect("reads");
    show::effects(&read).expect("parses")
}

fn fade() -> Transition {
    Transition {
        preset: "fade-smoothly".into(),
        duration: 1.5,
        auto: true,
        time: 4.0,
    }
}

#[test]
fn transitions_set_read_and_undo_as_one_step() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (_dir, mut worker, doc) = open_deck();
    assert_eq!(transition(&mut worker, &doc).preset, "");
    let before = steps(&mut worker, &doc);
    let json = show::transition_json(&fade(), false);
    worker
        .run(&doc, "slide.transition", &[&json])
        .expect("sets");
    assert_eq!(transition(&mut worker, &doc), fade());
    assert_eq!(steps(&mut worker, &doc), before + 1);
    worker.run(&doc, "edit.undo", &[]).expect("undoes");
    assert_eq!(transition(&mut worker, &doc).preset, "");
    // Every offered preset goes in and reads back.
    for preset in show::TRANSITIONS {
        let one = Transition {
            preset: preset.into(),
            ..fade()
        };
        let json = show::transition_json(&one, false);
        worker
            .run(&doc, "slide.transition", &[&json])
            .expect("sets");
        assert_eq!(transition(&mut worker, &doc).preset, preset);
    }
    let push = Transition {
        preset: "push-left".into(),
        ..fade()
    };
    let json = show::transition_json(&push, true);
    worker
        .run(&doc, "slide.transition", &[&json])
        .expect("sets all");
    doc.show_sheet(&mut worker, 1).expect("the second slide");
    assert_eq!(transition(&mut worker, &doc).preset, "push-left");
    let before = steps(&mut worker, &doc);
    for bad in [
        r#"{"preset":"no-such-thing"}"#,
        r#"{"preset":"dissolve","duration":0}"#,
        r#"{"preset":"dissolve","advance":"later"}"#,
        r#"{"preset":"dissolve","duration":"2"}"#,
        r#"{"preset":"dissolve","all":"yes"}"#,
    ] {
        assert!(
            worker.run(&doc, "slide.transition", &[bad]).is_err(),
            "{bad}"
        );
    }
    assert_eq!(steps(&mut worker, &doc), before);
}

#[test]
fn effects_add_change_move_and_go_as_single_steps() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (_dir, mut worker, doc) = open_deck();
    worker
        .run(&doc, "draw.shape", &["rectangle", "true"])
        .expect("draws");
    let objects =
        show::objects(&worker.values(&doc, show::OBJECTS).expect("reads")).expect("parses");
    let rect = objects
        .objects
        .iter()
        .position(|o| o.selected)
        .expect("the new shape");
    let add = |worker: &mut Worker, preset: &str| {
        let json = serde_json::json!({ "preset": preset }).to_string();
        worker.run(&doc, "animation.add", &[&json]).expect("adds");
    };
    let before = steps(&mut worker, &doc);
    add(&mut worker, "ooo-entrance-fade-in");
    assert_eq!(steps(&mut worker, &doc), before + 1);
    let list = effects(&mut worker, &doc);
    assert_eq!(list.len(), 1);
    let first = &list[0];
    assert_eq!(
        (
            first.preset.as_str(),
            first.class.as_str(),
            first.target,
            first.start.as_str()
        ),
        ("ooo-entrance-fade-in", "entrance", Some(rect), "click")
    );
    assert!(!first.hash.is_empty());
    add(&mut worker, "ooo-entrance-fly-in");
    let change = r#"{"index":1,"start":"after","duration":1.25,"delay":0.5}"#;
    let before = steps(&mut worker, &doc);
    worker
        .run(&doc, "animation.change", &[change])
        .expect("changes");
    assert_eq!(steps(&mut worker, &doc), before + 1);
    let second = effects(&mut worker, &doc)[1].clone();
    assert_eq!(
        (second.start.as_str(), second.duration, second.delay),
        ("after", 1.25, 0.5)
    );
    worker
        .run(&doc, "animation.change", &[r#"{"index":1,"move":-1}"#])
        .expect("moves");
    let order: Vec<String> = effects(&mut worker, &doc)
        .into_iter()
        .map(|e| e.preset)
        .collect();
    assert_eq!(order, ["ooo-entrance-fly-in", "ooo-entrance-fade-in"]);
    worker
        .run(&doc, "animation.change", &[r#"{"index":0,"remove":true}"#])
        .expect("removes");
    assert_eq!(effects(&mut worker, &doc).len(), 1);
    worker.run(&doc, "edit.undo", &[]).expect("undoes");
    assert_eq!(
        effects(&mut worker, &doc).len(),
        2,
        "the removal comes back"
    );
    // Every offered effect goes in.
    for preset in show::EFFECTS {
        add(&mut worker, preset);
        assert_eq!(
            effects(&mut worker, &doc).last().expect("one").preset,
            preset
        );
    }
    let before = steps(&mut worker, &doc);
    for (id, bad) in [
        ("animation.add", r#"{"preset":"no-such-effect"}"#),
        (
            "animation.add",
            r#"{"preset":"ooo-entrance-fade-in","duration":0}"#,
        ),
        ("animation.change", r#"{"index":99,"duration":1}"#),
        ("animation.change", r#"{"index":0,"start":"later"}"#),
        ("animation.change", r#"{"index":0,"move":-1}"#),
        ("animation.change", r#"{"index":0,"remove":"yes"}"#),
    ] {
        assert!(worker.run(&doc, id, &[bad]).is_err(), "{bad}");
    }
    assert_eq!(steps(&mut worker, &doc), before);
}

#[test]
fn an_animated_object_is_its_own_layer_under_its_effect_hash() {
    if install().is_none() {
        eprintln!("SKIP: no pinned LOK build");
        return;
    }
    let (_dir, mut worker, doc) = open_deck();
    worker
        .run(&doc, "draw.shape", &["rectangle", "true"])
        .expect("draws");
    let json = serde_json::json!({ "preset": "ooo-entrance-fade-in" }).to_string();
    worker.run(&doc, "animation.add", &[&json]).expect("adds");
    let hash = effects(&mut worker, &doc)[0].hash.clone();
    let layers = doc
        .slide_layers(&mut worker, 0, (480, 270))
        .expect("layers");
    assert_eq!(layers.size, (480, 270));
    let moving: Vec<(&str, bool, i64)> = (layers.list.iter())
        .filter_map(|layer| match &layer.part {
            show::Part::Moving {
                hash,
                visible,
                bounds,
            } => Some((hash.as_str(), *visible, bounds.2)),
            _ => None,
        })
        .collect();
    assert_eq!(moving.len(), 1);
    assert_eq!(
        (moving[0].0, moving[0].1),
        (hash.as_str(), false),
        "an entrance starts hidden"
    );
    assert!(moving[0].2 > 0, "it has a width");
    let still = layers
        .list
        .iter()
        .filter(|l| l.part == show::Part::Still)
        .count();
    assert!(still >= 2, "background and the rest");
    let bytes = 480 * 270 * 4;
    assert!(
        layers
            .list
            .iter()
            .all(|l| l.pixels.len() == bytes)
    );
}



