# YouCraft local development handoff — 2026-10-24

## Start here

User requests a smooth Windows CPU development loop now, GPU validation later on a Lenovo LOQ RTX 3050 6 GB. Priorities: reliable save/restart/update, then reaction calibration and editing. Do not rebuild/package the whole app for each small change. Discuss behavior before large architectural changes. Never run heavy CPU work on an HPC login node: use a confirmed Slurm allocation. This restriction does not prohibit development on the user's own laptop.

Canonical shared repository: https://github.com/routsourav1729/youcraft, branch `ama-bartalap`.
Baseline before this handoff: `3e946790b2c64b5856267a84b8abd22fa4575661`.
Public application: **0.1.11**, native build commit `ad2fa0a90071954fede47c3eb60c9d5d080a96a3`.
The later updater repair is in source and a separate release asset. The original 0.1.11 installer was NOT rebuilt with that repair.

This handoff is a source audit and implementation plan. Background automatic updates, restart/session restoration, calibrated reaction thresholds and timeline reaction markers below are NOT implemented by this document. Do not present them as shipped.

## Source ownership and synchronizing computers

In a normal GitHub clone:

- Rust app/UI: `apps/filmcraft`, `crates/ui-egui`.
- Python analysis: `ama-backend/backend/betun`.
- Python tests: `ama-backend/tests`.
- Windows updater/setup: `packaging/ama`.
- Installed UI automation: `ama-backend/tests/reaction_ui_smoke.py` and `stage_one_smoke.py`.
- GitHub Windows release build: `.github/workflows/ama-windows.yml`.
- Small Windows updater checks: `.github/workflows/updater-handoff.yml`.

The HPC workspace has a DIFFERENT layout: `/home/agipml/sourav.rout/MO/betun/vendor/filmcraft` contains Rust, while root `backend`, `tests`, `scripts`, `packaging` contain Python. `.local/youcraft` is the release checkout. **Do not overwrite newer local-PC commits by staging the old HPC source tree.** Fetch the user's branch first and reconcile changes. Prefer using the canonical Git clone for future development. Update the old source trees only deliberately after comparison.

Local workflow:

1. Clone the canonical repo/branch, then create `dev/windows-reactions` (or a task-specific branch).
2. Keep the Windows build target directory and dependencies between runs; do not use `cargo clean` routinely.
3. Commit/push small reviewable changes to the development branch. Share branch name + commit SHA + tests run with the HPC agent.
4. HPC agent fetches that exact commit for GPU tests; commits fixes to the same agreed task branch or a separate branch for review. Do not edit/push concurrently to the same branch without coordination.
5. Merge to `ama-bartalap` only when ready for its Windows release workflow. Ordinary development-branch pushes should not trigger full packaging.
6. Never commit recordings, model weights, environments, credentials, user databases, caches or target directories.

Do not follow `work/AMA-HANDOFF.txt`: that old HPC file described an early 0.1.0 build and is obsolete.

## Fast local build/test loop

Prerequisites: Windows Rust/MSVC build tools and the repo's pinned toolchain; Windows release CI currently uses Rust 1.95.0. Python 3.11 for analysis. Inspect workflow/build prerequisites before first compilation. The first native build may be substantial; subsequent debug builds reuse dependencies.

From repository root (on the user's Windows laptop):

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e './ama-backend[test]'
$env:BETUN_DATA_DIR = Join-Path $PWD '.dev-data'
.\.venv\Scripts\python.exe -m betun --port 8765
```

In a second shell at the same root, set the SAME `BETUN_DATA_DIR`, then:

```powershell
cargo +1.95.0 build --locked -p filmcraft
.\target\debug\filmcraft.exe
```

Run the development helper before the app; `local_backend.rs` recognizes the helper on port 8765 instead of requiring a newly packaged EXE. Close the installed production app first so its helper does not occupy that port. The backend writes its API token in the chosen data directory; do not print/publish it. Use a copied test project and an ignored `.dev-data` directory. Native UI cache is not fully isolated merely by BETUN_DATA_DIR; avoid recovery tests against production projects until a separate UI cache path is configured.

Model runtimes are separate environments. Reuse installed model configuration/weights; inspect `BETUN_MODEL_CONFIG` resolution in providers/configuration before launching inference. Do not install all speech/reaction packages into the core backend environment; MediaPipe uses its own pinned NumPy/runtime. CPU reaction model is intentional CPU execution, not silent CUDA fallback.

For Python-only changes, restart the source helper. No Rust build or installer is needed. For Rust UI changes, use the incremental debug build. For PowerShell-only changes, run the two updater checks with Windows PowerShell 5; no Rust compilation. Do not run the entire Rust workspace suite for a localized change.

Targeted checks, chosen according to the actual change:

```powershell
.\.venv\Scripts\python.exe -m pytest -q ama-backend/tests/test_reaction_flow.py ama-backend/tests/test_podcast_workflow.py ama-backend/tests/test_project_persistence.py
cargo +1.95.0 test --locked -p filmcraft-ui-egui --test podcast_ui
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging/tests/file-update.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File packaging/tests/updater-handoff.ps1
```

Use actual Windows UI automation/screenshots for layout and preview changes. Synthetic tests cannot establish gaze quality or audible playback on the user's machine. Validate a short representative recording before a full hour. Do not publish a new installer for every iteration.

## Priority 1: app-level updates, save, close and restore

Confirmed current defects/limitations:

- `crates/ui-egui/src/podcast.rs` places Updates, Audio output and Connect AI inside `if !s.projects_open`. Thus the home/project picker hides them. Models already sits outside that condition.
- `podcast_setup::launch_updater` starts a visible PowerShell console. There is no application-level background update service or restart-ready badge.
- Repaired updater requests normal window close, waits, and asks before stopping orphan helpers. It does NOT provide automatic save acknowledgement or relaunch.
- `State::checkpoint_on_exit` writes a draft but returns no Result and logs failure only. It cannot serve as a confirmed safe-to-update barrier.
- Native editor recovery and workflow project persistence are separate. `on_exit` checkpoints workflow data and flushes native persistence; merely calling a workflow save does not prove native timeline edits are saved.
- The updater downloads compressed changed FILES. A changed EXE is downloaded whole. This is not binary delta patching.

Required product behavior:

1. Home and every workspace show a compact Settings/Audio/Models area and update status. Keep project-only actions scoped to a project.
2. An app-owned update state machine: idle -> checking -> downloading -> verified/ready -> saving -> closing -> applying -> relaunching -> healthy (or failed/rolled back). Own it above project/page state so navigation cannot cancel it or start duplicate workers.
3. Check and download changed files in the background without blocking playback. Show version, progress, bytes, retry and a visible automatic-download preference. Defer on metered connections unless enabled.
4. When ready, offer **Restart to update** and **Later**. Never close the app merely because a download finished.
5. On Restart: pause transport; flush pending project edits and native editor persistence; record project ID/path, page, master playhead, layout and selection. Receive explicit success before closing. If save fails, leave the app open with the actual error. If analysis/export is active, defer or let the user explicitly cancel it; do not silently destroy the job.
6. Start an independent updater process, gracefully stop owned helpers, close the app, atomically apply the verified staged files, relaunch once, restore the same project/page/playhead paused. Persist session restore independently from project data. Avoid restart loops.
7. Keep rollback backups and transaction journal until startup health is confirmed; preserve diagnostics and recover safely on failure. Do not delete recovery data to satisfy cleanup.
8. Clean obsolete update-owned versions/partial files when superseded. Retain verified current-target downloads for retry. After successful installation/startup, remove staging/download/backup artifacts. Keep models, projects, source media and recovery drafts. Do not indiscriminately remove browser Downloads or user data.

Small first commit: expose existing update/audio actions on home. Next: acknowledged save/restore and updater protocol. Then background download and status UI. True binary deltas can come later: measure EXE changes, generate patches against known base hashes, verify result hash, retain whole-file fallback. Background download and binary deltas solve different problems.

Acceptance: home with no project can check/update; no duplicate downloads; canceled restart leaves app usable; save failure does not close it; offline/interrupted download resumes/reuses verified files; update failure rolls back; successful update relaunches the same dirty-before-save project with corrections intact; old owned cache removed only after success; models unchanged.

## Priority 2: explain and repair reactions

### What is actually installed

Default `Setup-vision.ps1` / `ama-backend/scripts/setup_vision.py` configures `mediapipe-reference` using MediaPipe Face Landmarker (face_landmarker.task), MediaPipe 0.10.21, OpenCV 4.11.0.86 and NumPy 1.26.4 in `vision-cpu`.

- Face Landmarker supplies landmarks/iris locations, expression blendshape coefficients, and a face transformation matrix.
- CPU “look” is OUR heuristic: compare normalized iris positions with a reference, plus a fixed 20-degree head-yaw gate. It does not know whom the host is looking at.
- Smile is the average mouthSmileLeft/mouthSmileRight coefficient. Current cutoff is hard-coded **> 0.35**. A visible smile at 0.22 therefore fails; the score is not a probability or calibrated percentage of smile strength.
- Nod is OUR temporal rule, not a dedicated trained nod detector: pitch range >= 7 degrees and end-to-start difference < 5 degrees within the current matched streak.
- Optional `mediapipe-l2cs` uses L2CS-Net/ResNet50 gaze pitch/yaw plus MediaPipe expressions. The default installer does not install/select it. Do not tell the user that L2CS is active without inspecting capabilities/model config.

Read `ama-backend/backend/betun/vision_worker.py`, `providers.py`, `pipeline.py`, `podcast.py`; UI in `crates/ui-egui/src/podcast.rs`.

### Confirmed source problems and limits

1. CPU tolerance is percentage of eye width; L2CS tolerance is angular deviation. Overlay instead displays absolute head pitch in degrees and smile as a percent. Show reference-look distance with its own units and threshold; show head motion separately. Never conflate eye position with head tilt.
2. Smile threshold is universal and hidden. Calibrate neutral and visible-smile examples for the actual person/camera. Show raw score plus calibrated decision, not “22% confident.”
3. Worker gates the temporal streak on matching gaze. A nod can break that gaze gate and disappear before the nod rule runs. Separate expression/head-motion extraction from the look eligibility filter; use short rolling windows, continuity checks and duration limits rather than one long streak's endpoints.
4. Worker samples around 8 Hz, but persists diagnostic measurements only once per second, without raw gaze direction or reference distance. That cache cannot support reliable nod/tolerance recalculation. Do not claim “adjust all controls without inference” until sufficient measurements are persisted in a bounded versioned analysis artifact. Avoid embedding hour-long dense data into the 8 MB project/UI payload.
5. Candidate worker accepts streaks of roughly 2 seconds, but selector requires at least 3 seconds; useful short cues can be discarded. Separate cue detection from editable shot duration and expand within valid guest-only coverage.
6. Current `eligible` requires ONE guest activity row to contain the whole candidate. Adjacent guest rows may reject a valid contiguous stretch; merge compatible activity intervals before checking. Preserve host speech, overlaps, locked shots and missing-media exclusions.
7. Screen-time overlay counts only `decision == keep`. Detection creates pending suggestions, so “2.3 min host / 88% guest” can remain unchanged after detection. Do not call that specific observed value a confirmed calculation error without the project. Label Current edit versus Proposed edit. Both-visible coverage may make host+guest percentages exceed 100%; show separate host-only/guest-only/both durations or explain visibility explicitly.
8. Stats are calculated from base shots plus reaction overlaps, independently from the composed output; they do not fully reflect approved cuts. Derive preview and stats from the same resolved timeline and use unioned intervals, avoiding double count.
9. Reaction regeneration replaces `project.reactions` wholesale; interval reselection preserves manual entries and decision history, but a new full analysis does not. Preserve manual edits/explicit rejections separately from model suggestions; invalidate only incompatible edits with an explanation.
10. Delete currently removes an entry without a durable rejection tombstone. It can return on interval reselection. Suggested Delete should persist rejection/disable state with Undo; manual Delete should remove the manual insertion. Stable candidate identities must survive selection changes.
11. Preview supports a pending reaction via `reaction_id`, and successful reaction jobs queue a review refresh. Therefore a frozen middle video is not proven to have one simple cause. Reproduce: generation -> project refresh -> revision -> review-plan -> native assembly -> decoder frame. Test stale selected IDs after deletion/reselection, save/request races, preserving playhead, and the expected difference between pending preview and committed edit. Do not overwrite a newer edit with an older asynchronous response.

### Proposed usable workflow and controls

Keep small host left with optional diagnostic overlay, large clean output center, small guest right. One master clock and resizable video area. No per-shard MOV list as the main editing surface.

Calibration strip: choose a short representative host-listening segment from the SAME camera angle; show several candidate frames for confirmation. Host-speaking samples are only an initial guess, not ground truth for attentive listening. Collect neutral and modest-smile examples; include an optional nod example. Require clear single face/visible eyes, reject blinks/occlusion and weak samples, allow screenshot reference for still look only. A still screenshot cannot calibrate temporal nod behavior. Recalibrate when camera angle changes; report unusable footage instead of inventing matches.

Main controls after calibration: look strictness, smile sensitivity, nod sensitivity, maximum insertion frequency, shot duration. Explain spacing as “minimum gap between reaction shots”; either derive it from frequency or place it under Advanced. Controls should regenerate suggestions from saved measurements where possible; distinguish “Recalculate suggestions” from expensive “Re-analyze video.” Always state whether an action changes only suggestions or replaces existing manual edits.

Review strip under player: colored spans for pending/kept/rejected/manual; click span to jump and preview; previous/next suggestion; Keep/Reject; Undo. Selecting a pending span temporarily auditions it and labels the center view accordingly. Keep/reject/delete updates output and statistics at the same project revision while preserving playhead and pause state.

Manual edits already have a basic “Add host shot here” button (3 seconds) and numeric start/end/Delete in the list. Improve to mark In/Out or drag a range -> “Show host here”; allow trim handles. Default manual action must respect valid host media, protected overlaps/locked edits and the master audio. Offer deliberate override separately if ever needed. Manual choices survive re-analysis. Do not force users to find/copy timestamps.

### Acceptance before calling gaze fixed

Use a small locally labeled set from this user's recording: neutral listening, modest smiles (including the 0.22 example), real nods, looking away, speaking mouth movements, blinks, occlusion, glasses glare if present, and camera-shard boundaries. Compare false positives and missed cues, not just total count. Show reasons for discarded spans (no face/look mismatch/no expression/protected speech/minimum length/frequency). Freeze a short regression fixture or anonymous measurements with permission; keep private footage off GitHub.

Test manual insertion/trim/delete/Undo across save, restart, interval changes and re-analysis. A rejected suggestion must not reappear. Test pending preview versus Keep, and recompute statistics from the same timeline as output. CPU is the first acceptance target. GPU work should use the exact agreed Git SHA and report actual device, model, memory, throughput, and OOM behavior; no silent CPU fallback. L40S results are not proof of 6 GB RTX 3050 behavior.

## Evidence and current validation

0.1.11 previously passed 95 backend tests, nine native UI/transport tests and installed Windows smoke checks. These tests do NOT certify real gaze accuracy, the user's current frozen-preview report, or RTX 3050 behavior.
Updater repair Windows checks: https://github.com/routsourav1729/youcraft/actions/runs/40073141425
Repair asset: https://github.com/routsourav1729/youcraft/releases/download/v0.1.11-preview.1/YouCraft-Updater-Repair.zip

Official references consulted:

- VS Code Windows updater downloads a versioned CodeSetup EXE and applies it in the background; installed disk footprint is not the download size: https://github.com/microsoft/vscode/blob/main/src/vs/platform/update/electron-main/updateService.win32.ts
- MediaPipe output definitions (landmarks, expression coefficients, transformation matrix): https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker
- Optional L2CS gaze estimator: https://github.com/Ahmednull/L2CS-Net

Suggested first instruction to local coding agent: Read this handoff and AGENTS.md, inspect the current branch/worktree, establish the incremental Windows debug loop with a copied test project, implement home update visibility and acknowledged save/restart first, then reproduce reaction preview/calibration problems on a short local segment. Keep implementation and test results separate from proposals. Do not start another full release until the user has reviewed the behavior locally.
