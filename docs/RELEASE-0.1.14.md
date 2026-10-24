YouCraft 0.1.14 repairs reaction review and reorganizes the guided workspace.

- Pending reactions preview in the output by default. Red spans identify pending
  shots, green spans kept shots, and blue spans split screen. Clicking a span
  seeks to it. Keep/Remove beside the player saves the decision and rebuilds the
  output; previewing alone never accepts a suggestion. Turn Preview suggestions
  off to compare the accepted edit. Export requires pending decisions to be resolved.
- Refresh suggestions replaces the misleading Apply suggestions label. Cached
  selection now accepts a guest sentence split across several diarization rows,
  including pauses up to 1.5 seconds. Host speech and protected cuts still exclude
  inserts. No model download or full analysis rerun is required for this correction.
- Reaction review uses the available width: review controls and shots on the
  left, detection and timing on the right, with independent scrolling. Reference
  selection uses the full width before analysis. Settings and timing are grouped;
  detailed signal calibration stays collapsed.
- New projects include all detected overlaps with a one-second buffer each side.
  Existing preferences are preserved; All interruptions · ±1s applies that preset.
  Overlap no longer extends through later silence because of minimum-shot rules.
  Split-screen output has priority over pending reactions.
- A camera starting a fraction of a frame late can no longer leave the subsequent
  output black until the first substantial speaker turn.
- Guided monitors expose playback and frame stepping. Lift, Extract, Insert,
  Overwrite, and editor marker controls remain in the full editor after handoff.
- Projects, Settings, Connect AI and Updates have a consistent global header.
  Audio output, models, diagnostics and project files live under Settings.
  The installed YouCraft version is visible in the header.
- Project rows show the saved name, actual stage and duration. New project folders
  use a readable name under Documents/YouCraft/Projects by default on Windows.
  Change default chooses the root for future projects; existing paths stay put.
- Project files allows renaming, metadata export and importing corrected metadata.
  Imports retain revision conflict and media-identity checks. Metadata contains
  transcripts, activity, camera cuts, calibration and reaction decisions; recordings
  remain linked at their original locations. The same actions are exposed through
  podcast.action as export_metadata/import_metadata with an absolute path.
- First local editor handoff saves edit.fcproj inside the project folder when that
  file does not already exist. Existing native edits are never overwritten by an
  automatic handoff save. Explicit native Save As also records its name and path
  in the workflow project.

Validation: focused backend regression suite, incremental native build, and local
UI checks of pending preview, Remove/save/rebuild, split-screen output and transport.
The Windows release workflow additionally tests the installed package and updater.

This keeps Rust/egui and the existing command/MCP engine. shadcn components require
a web frontend and are not a drop-in egui theme. Broader Audio/Speakers/Cameras
layout refinement and GPU adaptive batching remain separate work; no GPU speedup
is claimed for this release.
