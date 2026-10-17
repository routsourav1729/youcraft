# Editing with an assistant

Filmcraft's existing MCP bridge exposes editor actions to an AI client. It runs
locally. A skill describes the editing workflow; MCP gives the assistant controls.
Neither requires moving video files to a remote editing server.

Start the editor with `Start with MCP.cmd`. In a compatible local MCP client,
register the installed `filmcraft-cli.exe` with arguments:

```
mcp --bridge 127.0.0.1:9876
```

Example requests: “Show me the longest pauses,” “Preview a tighter opening,” or
“Export the current edit and Odia subtitles.” Inspect the proposed changes before
applying them. Filmcraft commands remain undoable; rendering happens locally.
The agent may send transcript text or requested screenshots to its model provider.
MCP itself does not make ChatGPT or Claude run offline.

The planned in-app Ask panel will reuse this bridge and show Preview, Apply and
Undo. It is not part of this V1 build. ChatGPT account integration is feasible
through OpenAI's documented Sign in with ChatGPT flow for open-source local apps
and Codex app-server. Eligibility and usage remain subject to the user's plan.

For Claude, an existing official Claude Code client can connect to the local MCP
bridge using the user's own login. A custom embedded Claude chat should use the
supported API/SDK authentication path. We do not copy account session tokens into
our app or pretend that logging into a website supplies API access.

Sources checked 17 October 2026:
- [OpenAI local-app sign-in](https://developers.openai.com/siwc/token-sharing-open-source)
- [Codex app-server](https://learn.chatgpt.com/docs/app-server)
- [Claude MCP](https://code.claude.com/docs/en/mcp)
- [Claude authentication and integration rules](https://code.claude.com/docs/en/legal-and-compliance)
