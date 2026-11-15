# T052 — Per-track synthesis engine (renderer) selection

**Status:** done  
**Owner:** Antigravity  
**Last updated (UTC):** 2026-11-15T10:15:00Z

## Goal

Implement authoritative per-track renderer selection (`track.set_renderer`) matching OpenUtau desktop behavior across the Rust `project-session` engine, serialization round-trip in `.ustx`, synth pipeline (`synth-cli`), server rendering worker (`synth-server`), and the Web React Dock frontend (`ui-mock/react-dock`).

### Architecture & Requirements

1. **Rust single source of truth**:
   - `domain::UTrack` stores `renderer_settings: Option<URenderSettings>`, where `URenderSettings.renderer` holds the engine identifier (e.g., `"r1"`, `"classic"`, `"r2"`, `"diffsinger"`, etc.).
   - `project-session`:
     - Added `track.set_renderer` command to `project-session/src/v2.rs` with full validation, trimming, and undo/redo support. Blank/null strings normalize to `None` (clearing track override to fall back to project default).
     - Exposed `renderer: Option<String>` in `TrackView` and `TrackMetaView` in `project-session/src/views.rs`.
     - Integration tests in `native/crates/project-session/tests/track_renderer.rs` cover set, change notification, undo/redo round-trip, null/whitespace clear, two-track independence, validation failure for invalid type/unknown ID, and YAML round-trip.
2. **Pipeline routing**:
   - `native/tools/synth-cli/src/pipeline/streaming.rs`:
     - Added `render_mixdown_stream_by_track_from_frame_with_engines` taking `engine_by_track: Option<&dyn Fn(usize) -> &'static dyn PhraseRenderer>`.
     - In streaming mixdown chunk processing, resolves `track_renderer` from `engine_by_track` per chunk's `track_no`, using it for chunk dynamics and streaming phrase synthesis.
   - `native/tools/synth-cli/src/pipeline.rs`:
     - Re-exported `render_mixdown_stream_by_track_from_frame_with_engines`.
     - Added `render_mixdown_with_pronunciations_and_engines`, delegating from `render_mixdown_with_pronunciations`.
   - `native/tools/synth-cli/tests/streaming_mixdown.rs`:
     - Added integration tests `streamed_mixdown_routes_per_track_engines` and `offline_mixdown_routes_per_track_engines` proving independent counting phrase renderers are invoked for each track.
3. **Synth server routing**:
   - `native/tools/synth-server/src/render_service.rs`:
     - Added engine resolution helper `resolve_engine(&name)` supporting `"r1"`, `"WORLDLINE-R"`, `"r1-rs"`, `"r2"`, `"WORLDLINE-R2"`, `"classic"`, `"diffsinger"`, `"moe"`.
     - In `RenderJob::RenderProject`, reads `track.renderer_settings.renderer` to resolve effective track engine.
     - In `RenderJob::RenderMixdown` and `RenderJob::RenderMixdownStream`, maps project tracks to per-track resolved renderers and routes via `engine_by_track`.
4. **Frontend UI (Web React Dock)**:
   - `ui-mock/react-dock/src/openutau/engine.js`:
     - Maps `track/renderer` intent to `{ type: 'track.set_renderer', track_id, renderer }`.
     - Maps `renderer: track.renderer ?? null` in snapshot emission.
   - `ui-mock/react-dock/src/openutau/clone-state.jsx`:
     - Reducer handles `track/renderer` action for instant preview response.
   - `ui-mock/react-dock/src/openutau/arrangement-panel.jsx`:
     - Added `RendererPicker` with dropdown choices (`Default engine`, `WORLDLINE-R (C++)`, `WORLDLINE-R (Pure Rust)`, `Classic Resampler`, `WORLDLINE-R2 (Neural)`, `DiffSinger`, `MoE Student`).
     - Rendered inside each track card in `.ou-track-info`.
   - `ui-mock/react-dock/src/openutau/openutau.css`:
     - Styled `.ou-renderer-select` matching `.ou-singer-select`.
   - `ui-mock/react-dock/tests/openutau-engine.test.js`:
     - Added unit tests for `track/renderer` intent mapping.

## Verification

- `cargo test --manifest-path native/Cargo.toml -p project-session`: 14 test targets, all tests passed (including all 7 `track_renderer.rs` tests).
- `cargo test --manifest-path native/Cargo.toml -p synth-cli --test streaming_mixdown`: all 10 unignored tests passed, including `streamed_mixdown_routes_per_track_engines` and `offline_mixdown_routes_per_track_engines`.
- `cargo test --manifest-path native/Cargo.toml -p synth-server`: 54 unit tests + 34 integration tests, all passed.
- `cd ui-mock/react-dock && node --test tests/*.test.js`: 165 tests (162 pass, 3 skipped, 0 fail).
- `cd ui-mock/react-dock && npm run build`: build succeeded cleanly.
