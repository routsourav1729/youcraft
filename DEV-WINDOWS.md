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

Leave this terminal open and launch the installed YouCraft. It detects the local
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
