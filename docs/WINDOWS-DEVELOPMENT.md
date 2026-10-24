# Windows source development

Use the canonical repository on `dev/windows-reactions`, based on `ae4b574`.
Rust 1.95.0 with the MSVC x64 toolchain, Visual Studio C++ Build Tools and a
Windows SDK are required. Python 3.11 uses the repository-local `.venv`:

```powershell
py -3.11 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -e './ama-backend[test]'
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/dev-windows.ps1
```

The launcher builds incrementally (three compiler jobs, debug symbols disabled),
starts the source helper on port 18765, and runs the editor with control port
19876. Use `-SkipBuild` when only Python changed. It refuses occupied ports and
stops only the helper it owns after its editor closes. Keep `target/` between runs.

Development data lives in `.dev-data/backend`, workflow drafts/preview cache in
`.dev-data/workflow`, and native recovery/preferences in `.dev-data/native`.
Keep `.venv/`, `.dev-data/`, `work/`, and Python egg-info excluded locally.
The installed `YouCraft/models.json` is reused without modifying model runtimes.
Only use copied test projects. Do not package a release for each source change.

`YOUCRAFT_BACKEND_URL` selects an already-running local helper; an unavailable
explicit helper reports an error rather than starting a packaged helper.
`YOUCRAFT_UI_CACHE_DIR` overrides workflow cache. Explicit `--data-dir` or
`FILMCRAFT_DATA_DIR` also isolates eframe preferences. Default installed paths
remain unchanged when these overrides are absent.

Updates, Audio output and Connect AI are available from Home. The source build
does not carry the packaged updater: update source with Git and rebuild. Do not
copy the release updater beside the debug executable, where it could replace
development binaries with a published release.

## Next reaction workflow changes (not yet implemented)

Read-only audit of the user's saved 20-minute episode on 2026-10-24: 21 cached
candidates, but the current selector returns zero suggestions at 15, 30, 60 and
120-second intervals. Five candidates are shorter than three seconds, four lack
a smile/nod cue, and the other twelve overlap host speech or its half-second
safety margin. The sole kept reaction is a manual three-second insertion.
All eight referenced media files exist. These findings explain why interval
changes alone cannot fix this particular episode; they do not validate the
underlying face measurements or speaker labels.

The backend edit-review plan includes the kept manual reaction on camera A.
Its source-time breakdown is 136.6 seconds host-only, approximately 1047.49 seconds
guest-only, 5.6 seconds both-visible, and 10.31 seconds with no center picture
(142.2 seconds total host visibility). The opening saved shots explicitly have
layout `black`; side-camera frames are visible there. Do not count empty center
spans as guest visibility. Exported timing after approved cuts is a separate check.

Live verification: seeking to 115 seconds shows the kept manual host reaction in
the center. Monitor frame and playhead frame both equal 2875; monitor revision and
project revision both equal 572. This does not certify refresh behavior after
re-analysis, rapid edits, or rejection/reselection. Zero automatic suggestions
remain a separate selection issue.

A separate copy lives in the development backend store. Dry-running shot
generation with all detected overlaps and one-second padding changes both-camera
coverage from roughly 5.6 to 300.7 seconds. This was not saved back to either edit;
review the speech labels before treating every short overlap as ground truth.

- No fixed host-visibility target. Show current and proposed host-only,
  guest-only and both-visible minutes while adjusting suggestion controls.
- Resolve preview, statistics and output from the same revision/timeline.
  Pending auditions must be labeled; saved Keep/Reject choices must refresh
  the output while preserving playhead and pause state.
- Preserve genuine short overlaps. Use one-second padding on each side,
  configurable by the user; do not silently reset it during reaction analysis.
  Preserve locked edits and report missing camera coverage. A brief solo turn
  is not automatically evidence of overlapping speech.
- Separate look-distance, smile signal and temporal nod evidence; calibrate
  several neutral/listening and modest-smile samples from the same camera.
  Rank suggestions per requested interval and show why candidates were excluded.
  Blendshape scores are not calibrated probabilities.
- Store bounded, versioned measurements separately from project JSON so
  sensitivity changes can recompute suggestions without repeating inference.
- Preserve manual reactions and rejection history during re-analysis. Add a
  review strip, range insertion, trimming, Keep/Reject and Undo.

## GPU execution design (not yet implemented or benchmarked)

Detect runtime compatibility, driver, free VRAM, RAM and disk before offering
CPU/NVIDIA runtime installation. Download pinned, verified model revisions with
visible size/progress and user consent. Reuse existing compatible installations.
Keep the lightweight MediaPipe CPU path; benchmark alternatives before replacing it.

Plan memory using current free VRAM, not merely the card's nominal capacity.
Reserve display/decoder headroom, serialize heavy model stages, use supported
precision, bounded streaming/chunks and model-specific batch sizes. Retry bounded
OOM failures with smaller work units and preserve completed results. Report an
unsupported configuration when the model itself cannot fit; offer explicit CPU
fallback rather than silently switching devices. Independent diarization chunks
must preserve speaker identities across boundaries.

The current PC has an Intel i7-1360P, about 16 GB RAM and Iris Xe graphics. CPU
checks here cannot establish RTX 3050 6 GB speed or OOM behavior. Validate the exact
commit on that laptop before claiming a supported GPU profile.

Primary references consulted:

- https://docs.pytorch.org/docs/stable/generated/torch.cuda.memory.mem_get_info.html
- https://docs.pytorch.org/docs/stable/generated/torch.cuda.OutOfMemoryError.html
- https://developers.google.com/edge/mediapipe/solutions/vision/face_landmarker/python
- https://huggingface.co/nvidia/Nemotron-3-Diarization
- https://github.com/Ahmednull/L2CS-Net

## Validation on this laptop

- Rust 1.95.0, Visual Studio 2022 Build Tools 17.14.42, and Windows SDK
  10.0.26100 installed successfully.
- `cargo +1.95.0 build --locked -p filmcraft` succeeded with the launcher profile.
  Existing vendored CPAL warnings remain.
- Targeted `podcast_ui::startup_lands_on_clean_projects_screen`: passed.
- Actual running source app: Home controls visible, Audio dialog opens/closes,
  Updates explains source-build behavior, and project storage is isolated.
- Screenshots of Home, Audio and the copied episode's manual reaction were
  inspected locally; private media screenshots stay in ignored `work/`.
- Formatting and Git whitespace checks passed for the initial setup. No release packaging or push at that stage.

## Follow-up: upstream sync and update workflow (24 October 2026)

The working tree now integrates upstream FilmCraft 0.12.0 through
`057f460085c6c1c1f2335f1ba1c599a2ff36da67`, starting from the imported
0.8.0 snapshot recorded in the old UPSTREAM_REVISION. Because the fork has no
shared Git ancestry, this was a three-way source import, preserving YouCraft's
branding, Podcast workspace, Odia translations and Windows release workflow.

Home now exposes Audio output, Connect AI and Updates. Installed builds prepare
verified changed files in the background and offer an explicit restart. Restart
waits for the workflow save acknowledgement, saves the native timeline and UI
state, then closes normally. Transaction backups remain until the new app and
helper acknowledge startup. Existing 0.1.11 installations require one manual
update to obtain this new behavior. The 0.1.12 feed is not published merely by
building the source: the release branch workflow must finish successfully.

The authenticated MCP bridge supports the custom workspace and native editor in
the same application. A separate render/export is not required to continue
editing the generated native timeline. The bridge derives its local credential
from the existing private helper secret; it never exposes an anonymous editor
control server.

Fast verification completed for this follow-up:

- Cached native editor and CLI build passed; the final incremental build took
  approximately 65 seconds. Existing vendored CPAL warnings remain.
- 34 focused persistence, reaction-flow and workflow Python tests passed.
- File-update, manual-updater handoff and background-updater PowerShell fixture
  tests passed, including verified staging, retry, backup retention and rollback.
- Packaging fixtures verified fresh-install version records, deterministic
  manifests, hash validation and refusal to overwrite a published version.
- Actual source app checks verified Home controls, authenticated MCP discovery
  (17 tools) and inspection of the copied real episode. This also reproduced and
  fixed a preflight response-size bug for large episode JSON.
- An intentionally blocked native snapshot returned an error while the editor
  remained usable. A successful snapshot saved native and workflow state without
  embedding the helper credential.
- A new source process restored the copied episode, reaction page, workspace and
  exact 115-second playhead, paused, and wrote the helper startup acknowledgement.
- Initial Home sizing was visually checked without resizing/minimize/maximize.
  A later visible restart exposed a missing review-player flag despite correct
  native state. Restart snapshots now preserve preview identity and mode, and
  refresh model capabilities without leaving the restored page. The development
  launcher suppresses the debug console without hiding the editor window.
- A fresh close/reopen with the correction was visually verified: host and guest
  cameras, the edited center output, 01:55 playhead and installed model state all
  restored without pressing Reload. The source app was left open on this copied
  development episode.
- All three focused Rust update tests passed, including active-preview restoration,
  failed saves and active-job protection. The first test-cache build took about
  nine minutes; execution itself took 0.03 seconds.

Full codec suites, model reinstalls and local release packaging were intentionally
omitted for fast iteration. The focused tests and live checkpoint/restore checks
above are the executed evidence. Packaged
release checks remain in CI. GPU throughput and reaction usability changes remain
separate work; this machine cannot validate an RTX 3050 profile.
