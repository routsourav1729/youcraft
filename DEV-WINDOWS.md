# Work on YouCraft locally (Windows)

Use the `ama-bartalap` branch:

```powershell
git clone --branch ama-bartalap https://github.com/routsourav1729/youcraft.git
cd youcraft
```

## Fast backend fixes with the installed app

Install Python 3.11. Close YouCraft first, then run from the checkout:

```powershell
py -3.11 -m venv ama-backend\.venv
ama-backend\.venv\Scripts\python.exe -m pip install -e .\ama-backend
ama-backend\.venv\Scripts\python.exe -m betun
```

Native synchronization requires the matching CLI from this source version (the
0.1.5 installer does not provide `sync-audio`). After building below, set this in
the backend terminal **before** starting Python:

```powershell
$env:YOUCRAFT_CLI = (Resolve-Path target\debug\filmcraft-cli.exe).Path
```

Leave this terminal open and launch the matching YouCraft build. It detects the local
helper on port 8765. Model packs and existing projects are reused from your user
profile; no model reinstall is required. After Python edits, finish/cancel any
active analysis and restart that terminal command. This avoids rebuilding the EXE.

## Change the native UI

Install Rust 1.95.0 with the MSVC toolchain and Visual Studio Build Tools (Desktop
development with C++ and Windows SDK). Keep the backend above running. In a second
PowerShell at the checkout root:

```powershell
rustup toolchain install 1.95.0 --profile minimal
$env:AMA_DISABLE_LOCAL_BACKEND = '1'
cargo +1.95.0 build --locked -p filmcraft -p filmcraft-cli
Copy-Item packaging\ama\*.ps1 target\debug\ -Force
Copy-Item ama-backend\scripts\*.py target\debug\ -Force
& target\debug\filmcraft.exe
```

Subsequent UI changes need only the cargo build command and an app restart. Close
the installed app before starting this one: only one editor should own MCP port
9876. Register Claude again if switching between the installed and development
MCP executable; registration points to the current executable's folder.

The editable application code is in `crates/ui-egui/src/podcast.rs`; the native
review timeline is in `crates/engine/src/podcast.rs`; analysis and project storage
are in `ama-backend/backend/betun`. Do not commit recordings, model weights or tokens.

## Claude Code app versus terminal CLI

In YouCraft choose **Connect AI → Register Claude app** for the app's Local Code
sessions. Quit Claude fully, reopen it, choose **Local**, select this checkout as
the project folder, and enable YouCraft among the session tools. The registration
button configures tools; it does not launch Claude. **Register Claude CLI** is for
the separately installed `claude` terminal command.

To repair registration in an already installed preview without opening an MCPB file:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$env:LOCALAPPDATA\Programs\YouCraft\Connect-AI.ps1" -Client desktop
```

That command assumes the default installation folder; change the path if you
installed elsewhere. Keep YouCraft open while Claude uses the tools. Use **Check
tools** to verify the YouCraft side; this does not prove that Claude has enabled
them in your session. Browser/cloud chats cannot directly start this local stdio
helper. A .mcpb opening in Notepad is a Windows file association issue, not an MCP
connection; registration avoids that association.

Official reference: https://code.claude.com/docs/en/desktop#mcp-servers-from-the-claude-desktop-chat-app

## App updates

From 0.1.7, use **Updates** in the Podcast header. A separate Windows progress
window checks the published version, downloads from the Pages mirror, verifies
size and SHA-256, and opens the installer after confirmation. Save and close
YouCraft before installation. Install over the current app; do not uninstall.
Models and project folders are retained. A failed download removes its partial
file and leaves the existing installation intact.

Publishing an update requires refreshing `update.json` on the `downloads` branch
with `version` (numeric three-part version), `size` (bytes), and `sha256`, alongside
the matching `YouCraft-Setup-x64.exe`. Increment the updater's installed version
and Inno Setup AppVersion together. The downloader uses a fixed HTTPS origin.
The checksum detects incomplete/mismatched downloads; it is not code signing.

## Stage-by-stage development (0.1.8 onward)

Pushes build the installer, check project persistence, and exercise the installed
Projects/Audio-output UI through the existing control server. They do not compile
an additional native unit-test executable or download speech models. Manually
select `full_checks` for the complete backend/native suite, or `full_setup` to
validate model installation. Keep each feedback release scoped to one workflow
stage. Rust changes still require a native build; backend-only iteration can use
the development helper described above without rebuilding the app.

Launch opens Projects. Selecting Continue opens that project's saved view (or
its earliest useful review step for older projects). New project clears the
current inputs without deleting the previous episode. Active analysis or unsaved
changes must finish before switching. Opening an explicit Filmcraft project file
still opens that edit.

New Windows projects use the system Documents folder under `YouCraft/Projects`;
redirected Documents folders are respected. The Projects page shows the root and
each episode's actual directory. Project folder allows a user-selected location;
existing directories remain unchanged by updates. The legacy project index stays
in `%LOCALAPPDATA%/Betun/betun.sqlite3`, model/runtime packs in
`%LOCALAPPDATA%/YouCraft`, and per-project view preferences in
`%LOCALAPPDATA%/YouCraft/media/view-<project-id>.json`. These are user data, separate
from the replaceable installation in `%LOCALAPPDATA%/Programs/YouCraft`.

For audio feedback, use Audio output → Test sound, then Copy diagnostics. The test
uses the app's actual output stream, independent of media/model setup. A successful
stream callback is not a claim that the user heard sound; confirm that manually.

## File-level updates (0.1.9 onward)

0.1.8 needs one final full installer upgrade to acquire the new updater. From
0.1.9, Updates compares installed SHA-256 hashes with `files-VERSION.json` and
fetches only changed files from immutable `files/HASH.gz` objects on the same
HTTPS mirror. A changed EXE is downloaded whole; this is not binary delta patching.
New files are staged and verified before application. All replaced files have
backups and a persistent transaction journal. Close the editor before applying.
If interrupted, rerun Updates with the editor closed to restore the prior files.
Successful updates delete their staging and backup cache. Full installers remain
available for repair. Models, projects and runtime packs outside the installation
are untouched. Hashes detect corruption; packages are not code-signed.

The old full-installer cache is one reused file, not a growing series, at
`%LOCALAPPDATA%\YouCraft\updates\YouCraft-Setup-x64.exe`. The new updater removes
that old cached download when next opened. Subsequent file updates write their
managed file list to `installed-files.json`; only previously managed paths can
be removed. Windows Add/Remove Programs retains the last full-installer version;
the updater's installed version identifies the actual application update.

## 0.1.10 audio and shard review

Every shard searches the continuous mono master independently using Filmcraft's
existing GCC-PHAT matcher. A unique global anchor narrows subsequent searches;
clock drift is fitted from multiple anchors. Camera picker order cannot constrain
matches. Failed matches clear stale evidence. The visible confidence ratio is not
a percentage; use the matched master interval, anchor-fit error and master-audio
preview to review synchronization. Each shard preview explicitly selects its video.

The Windows CPAL 0.16 dependency has a narrow vendored compatibility patch: prime
the cold shared output buffer before Start and use a thread handle when setting
audio thread priority. Startup without callbacks gets bounded retries and clearer
endpoint/frame-count diagnostics. This still requires an audible Realtek laptop
retest; hosted CI cannot certify the user's speakers.

Compute-node checks: 88 backend tests; isolated native watchdog regression; all
seven real camera-audio shards from the prepared 20-minute bundle, reversed, with
start offsets within 14 ms of the known placements. Matching took about 25 seconds
on the compute node. This is not a laptop benchmark.

## 0.1.11 reaction workspace and seeking

Full model setup now includes the CPU reaction runtime; the Audio page also exposes missing reaction setup before editing. Reference suggestions prioritize solo host speech near the middle of each recording, retain partial successes, and accept an explicit host screenshot for a consistent camera angle. Calibration and measurements are saved in the project. MediaPipe startup warnings are not shown as the actionable failure; a missing face has a recovery instruction.

The reaction workspace uses one master clock, smaller host/guest views flanking the output, a draggable video/control divider, horizontal tolerance/interval/gap controls, and measured eye/head cues only on the host view. Detection markers are sampled once per second, not live frame-by-frame tracking. Screen share counts accepted inserts and both views during overlap. Default overlap padding is two seconds; explicit reaction generation applies it while retaining locked cuts. Pending suggestions can be auditioned without accepting them. Interval changes reuse saved candidates and preserve decisions; reference/tolerance edits require explicit regeneration.

Monitor dragging suspends playback once and resumes once on release. A pause cancels pending resume and does not trigger a stray scrub grain. Real Windows speaker audibility still needs laptop feedback; CI verifies transport state and installed UI behavior.
