# YouCraft 0.1.15

## Guided workspace

- Audio separates the recording from processing settings. Speakers uses paired
  role cards. Cameras groups host and guest recordings in two columns, stacking
  at narrow widths. Primary actions are gated by the required inputs.
- Camera preview height is independently resizable. Transcript and Reactions
  retain their shared larger preview size.
- The native editor header, window icon, Import view and About use YouCraft
  branding. About reports the YouCraft release version. Upstream attribution
  and open-source notices remain intact.

## Split-screen framing

- Output stays 1920×1080. Fill frame gives each person a 960×1080 crop, preserving
  aspect ratio. Fit full image retains letterboxing. Single-camera shots stay
  unchanged. Crop effects remain editable in the native timeline.
- Suggest framing samples nine frames per recording using the installed
  MediaPipe face model on CPU. A robust center is held for the entire recording;
  ambiguous or insufficient detections retain the previous center. This is
  stable framing for seated podcasts, not continuous person tracking.
- Horizontal/vertical controls and Framing confirmed are available per recording.
  Confirmed recordings are excluded from subsequent suggestions. Preview split
  screen lets users check the composition before transcript/reaction processing.
- Preview, native handoff and direct export use the same framing values. Overlap
  retains priority over kept reaction shots in the native handoff.
- Existing native edits and already-running exports are not rewritten. To use
  new framing, review Cameras and hand off the updated guided edit, or adjust the
  existing native timeline explicitly.

## AI and setup

MCP exposes the same actions as the UI, including `podcast.action` with
`action: framing`, and per-recording `frame_x`, `frame_y`, `framing_reviewed`
through `podcast.review` clips. `settings.split_framing` accepts `fill` or `fit`.
Connect AI registers the installed bridge for the selected client. Restart that
client, enable YouCraft tools, and keep YouCraft open. Installing the editor alone
does not register it in every AI application.

Install all models already runs speaker, transcript and face-model setup in
sequence. Framing introduces no additional model or GPU runtime.

## Validation

Targeted backend regression tests; native pixel checks of both split panes in
preview and 1920×1080 output; real-recording CPU framing; native page interaction
and screenshots; short H.264/AAC export. The broad Rust unit-test invocation was
stopped because it rebuilt a separate dependency set during the user's full
episode export; equivalent crop checks ran through the built MCP/engine instead.

Design references: [shadcn installation](https://ui.shadcn.com/docs/installation),
[MediaPipe face landmarks](https://ai.google.dev/edge/mediapipe/solutions/vision/face_landmarker).
No web frontend migration, shadcn runtime, or new global design skill was installed.
