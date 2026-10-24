# YouCraft 0.1.16

- Missing camera coverage no longer blocks native-editor handoff. Uncovered
  portions retain master audio over black video as editable timeline gaps.
  One available camera fills the frame; both cameras are not required throughout.
  Only explicitly approved cuts remove source time.
- Reactions has Check sync, and handoff requires confirmation of every camera
  shard. Camera pictures use the same master-clock mapping in preview and export.
- Ignore zero-length segments caused by floating-point shard boundaries.
- The native editor's Podcast button is now labelled Back to project.

Validation covers uncovered heads/tails, single-camera coverage, explicit cuts,
clock drift, shard boundaries, and matching preview/export source times. Camera
audio remains excluded; the separate master supplies the timeline audio.
