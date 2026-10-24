"""Read-only MCP preflight; no dependency on console encodings or a speech runtime."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
from .media import external_process


def check(executable, tools_only=False, timeout=15):
    executable = str(Path(executable).resolve(strict=True))
    with tempfile.TemporaryFile() as errors:
        process = external_process([executable, 'mcp', '--bridge', '127.0.0.1:9876'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
        replies = queue.Queue(maxsize=64)
        stopped = threading.Event()

        def read():
            while not stopped.is_set():
                # A real episode includes transcript and reaction measurements. Do not split
                # its JSON response at the old 1 MiB boundary and report invalid JSON.
                line = process.stdout.readline(16 * 1024 * 1024 + 1)
                try:
                    replies.put(line, timeout=1)
                except queue.Full:
                    return
                if not line:
                    return

        thread = threading.Thread(target=read, daemon=True)
        thread.start()

        def send(message):
            process.stdin.write((json.dumps(message, ensure_ascii=True) + '\n').encode('utf-8'))
            process.stdin.flush()

        def ask(identifier, method, params):
            send(dict(jsonrpc='2.0', id=identifier, method=method, params=params))
            deadline = time.monotonic() + timeout
            for _ in range(64):
                try:
                    line = replies.get(timeout=max(.01, deadline - time.monotonic()))
                except queue.Empty:
                    raise RuntimeError(f'MCP did not answer {method}. Keep YouCraft open and retry.') from None
                if not line:
                    raise RuntimeError('MCP executable stopped before replying.')
                if len(line) > 16 * 1024 * 1024 or not line.endswith(b'\n'):
                    raise RuntimeError('MCP response exceeded the supported 16 MiB limit.')
                reply = json.loads(line)
                if reply.get('error'):
                    raise RuntimeError(f'MCP error: {reply["error"]}')
                if reply.get('id') == identifier:
                    return reply['result']
            raise RuntimeError('Too many unrelated MCP messages.')

        try:
            hello = ask(1, 'initialize', dict(protocolVersion='2025-03-26', capabilities={},
                clientInfo=dict(name='youcraft-check', version='0.1.3')))
            if not hello.get('serverInfo', {}).get('name'):
                raise RuntimeError('MCP initialization failed.')
            send(dict(jsonrpc='2.0', method='notifications/initialized'))
            tools = ask(2, 'tools/list', {})['tools']
            if 'ui_control' not in [tool['name'] for tool in tools]:
                raise RuntimeError('Podcast control tool is missing.')
            if not tools_only:
                result = ask(3, 'tools/call', dict(name='ui_control',
                    arguments=dict(method='podcast.inspect', params={})))
                if result.get('isError'):
                    raise RuntimeError(' '.join(c.get('text', '') for c in result.get('content', [])))
                episode = json.loads(result['content'][0]['text'])
                if not isinstance(episode, dict) or 'step' not in episode:
                    raise RuntimeError('Running app did not return a podcast workflow.')
            return dict(tools=len(tools), editor_checked=not tools_only)
        finally:
            stopped.set()
            if process.poll() is None:
                process.kill()
            process.wait(timeout=5)
            thread.join(timeout=2)
            process.stdin.close()
            process.stdout.close()
