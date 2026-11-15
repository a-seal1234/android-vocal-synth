//! `track.set_renderer` — the per-track renderer / engine control.
//!
//! Exposes per-track renderer selection (r1, r1-rs, classic, etc.) in the
//! authoritative project session. Tests cover: set, clear with null or whitespace,
//! undo/redo round-trip, per-track independence, invalid type / unknown id handling,
//! and USTX YAML round-trip.

use project_session::v2::{MutationResult, ERR_NOT_FOUND};
use project_session::Session;
use serde_json::{json, Value};

const DEMO: &str = include_str!("../../../tools/synth-cli/tests/data/demo-song.ustx");

const TWO_TRACKS: &str = r#"ustx_version: 0.9
name: Two Tracks
bpm: 120
beat_per_bar: 4
beat_unit: 4
time_signatures:
- bar_position: 0
  beat_per_bar: 4
  beat_unit: 4
tempos:
- position: 0
  bpm: 120.0
tracks:
- track_name: Lead
- track_name: Harmony
voice_parts:
- name: LeadPart
  track_no: 0
  position: 0
  duration: 480
  notes:
  - position: 0
    duration: 480
    tone: 60
    lyric: la
"#;

fn request(revision: u64, command: Value) -> String {
    json!({ "expected_revision": revision, "command": command }).to_string()
}

fn snapshot(session: &Session) -> Value {
    serde_json::from_str(&session.snapshot_json().expect("snapshot")).expect("json")
}

fn track_id(session: &Session, index: usize) -> String {
    session.identity().tracks[index].id.as_str().to_string()
}

/// `renderer` of every track, in document order.
fn renderers(session: &Session) -> Vec<Value> {
    snapshot(session)["project"]["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .map(|track| track["renderer"].clone())
        .collect()
}

/// Apply a renderer change, returning the envelope so a test can read `changes`.
fn set_renderer(session: &mut Session, index: usize, renderer: Value) -> MutationResult {
    let revision = session.revision();
    let id = track_id(session, index);
    session
        .apply_v2(&request(
            revision,
            json!({"type": "track.set_renderer", "track_id": id, "renderer": renderer}),
        ))
        .unwrap_or_else(|error| panic!("track.set_renderer failed: {error:?}"))
}

#[test]
fn a_track_renderer_is_set_and_the_change_names_the_track() {
    let mut session = Session::open(DEMO, None, 1).expect("open");
    let id = track_id(&session, 0);

    let result = set_renderer(&mut session, 0, json!("r1-rs"));

    assert_eq!(session.revision(), 1);
    assert_eq!(renderers(&session)[0], json!("r1-rs"));
    assert_eq!(result.command_name, "Track renderer");
    assert!(
        result
            .changes
            .iter()
            .any(|change| change.kind == "track.changed"
                && change.track_id.as_deref() == Some(id.as_str())),
        "expected a track.changed change for {id}, got {:?}",
        result.changes
    );
}

#[test]
fn the_change_undoes_and_redoes_as_one_transaction() {
    let mut session = Session::open(DEMO, None, 1).expect("open");
    let before = renderers(&session)[0].clone();

    set_renderer(&mut session, 0, json!("classic"));
    assert_ne!(renderers(&session)[0], before);

    session.undo_v2(1).expect("undo");
    assert_eq!(renderers(&session)[0], before);

    session.redo_v2(2).expect("redo");
    assert_eq!(renderers(&session)[0], json!("classic"));
}

#[test]
fn null_clears_the_renderer() {
    let mut session = Session::open(DEMO, None, 1).expect("open");

    set_renderer(&mut session, 0, json!("r1-rs"));
    assert_eq!(renderers(&session)[0], json!("r1-rs"));

    set_renderer(&mut session, 0, Value::Null);
    assert_eq!(renderers(&session)[0], Value::Null);
}

#[test]
fn an_empty_or_whitespace_renderer_normalises_to_none() {
    let mut session = Session::open(DEMO, None, 1).expect("open");

    set_renderer(&mut session, 0, json!("   "));
    assert_eq!(renderers(&session)[0], Value::Null);
}

#[test]
fn two_tracks_hold_independent_renderers() {
    let mut session = Session::open(TWO_TRACKS, None, 1).expect("open");

    set_renderer(&mut session, 0, json!("r1-rs"));
    set_renderer(&mut session, 1, json!("classic"));

    let all = renderers(&session);
    assert_eq!(all[0], json!("r1-rs"));
    assert_eq!(all[1], json!("classic"));
}

#[test]
fn a_rejected_renderer_change_leaves_the_document_untouched() {
    let mut session = Session::open(DEMO, None, 1).expect("open");
    let before_yaml = session.yaml();
    let before_renderers = renderers(&session);
    let before_ids = session.identity().clone();

    // Wrong field type: neither a string nor null.
    let err = session
        .apply_v2(&request(
            0,
            json!({"type": "track.set_renderer", "track_id": track_id(&session, 0), "renderer": 7}),
        ))
        .expect_err("a number is not a renderer");
    assert_eq!(err.code(), "validation_error");

    // Unknown track id.
    let err = session
        .apply_v2(&request(
            0,
            json!({"type": "track.set_renderer", "track_id": "no-such-track", "renderer": "r1-rs"}),
        ))
        .expect_err("unknown track");
    assert_eq!(err.code(), ERR_NOT_FOUND);

    assert_eq!(session.revision(), 0, "no command committed");
    assert_eq!(session.yaml(), before_yaml);
    assert_eq!(renderers(&session), before_renderers);
    assert_eq!(session.identity(), &before_ids);
}

#[test]
fn the_renderer_survives_a_yaml_round_trip() {
    let mut session = Session::open(DEMO, None, 1).expect("open");
    set_renderer(&mut session, 0, json!("r1-rs"));

    let yaml = session.yaml();
    let reopened = Session::open(&yaml, None, 1).expect("reopen");
    assert_eq!(renderers(&reopened)[0], json!("r1-rs"));
}
