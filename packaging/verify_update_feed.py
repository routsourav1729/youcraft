"""Bounded post-deployment check: success means the public feed and manifest agree."""
import json
import sys
import time
import urllib.request

version = sys.argv[1]
base = "https://routsourav1729.github.io/youcraft"
for attempt in range(40):
    try:
        def read(name):
            with urllib.request.urlopen(f"{base}/{name}?check={time.time_ns()}", timeout=15) as response:
                return json.load(response)
        feed = read("update.json")
        manifest = read(f"files-{version}.json")
        if feed["version"] == version and manifest["version"] == version and manifest["files"]:
            print(f"Public update feed verified: {version}")
            break
    except (OSError, ValueError, KeyError):
        pass
    time.sleep(15)
else:
    raise SystemExit("Pages has not published the expected version; do not report this release as delivered")
