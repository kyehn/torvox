#!/usr/bin/env python3
"""pyte reference: parse VT sequence (hex-encoded), output JSON snapshot."""
import sys, json
from pyte import Screen, Stream

hex_seq = sys.argv[1]
seq = bytes.fromhex(hex_seq).decode("latin-1")
s = Screen(80, 24)
stream = Stream(s)
stream.feed(seq)
snap = {
    "lines": list(s.display),
    "cursor": {"x": s.cursor.x, "y": s.cursor.y},
}
print(json.dumps(snap))
