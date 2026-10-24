YouCraft 0.1.12 preview integrates FilmCraft changes through 057f460085c6c1c1f2335f1ba1c599a2ff36da67 (FilmCraft 0.12.0).

- Audio output, Connect AI and Updates are available from the project-selection screen.
- Installed releases check and download verified changed files in the background. Restart remains an explicit action.
- Restart saves the podcast project and native timeline, preserves the page/workspace/playhead, and resumes paused. Failed saves and active jobs block restart. Update backups remain until restored startup is acknowledged.
- Startup uses the current display and a smaller minimum window size for scaled laptop screens.
- MCP retains upstream's authenticated control channel; YouCraft's installed bridge derives its credential from the existing private local helper secret, so users do not copy tokens.
- New installations include their update version manifest. The release workflow publishes the installer and incremental feed after packaged checks pass, then verifies the public feed.

Existing 0.1.11 installations need one manual update to receive the new background updater. Large model downloads and repeated model setup are not part of this update. Reaction calibration and adaptive GPU inference are separate work and are not claimed as fixed here.
