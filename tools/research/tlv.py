"""Quick TLV walker for exploring .xdw files (research only)."""
import sys

def read_len(b, i):
    n = b[i]
    if n < 0x80:
        return n, i + 1
    k = n & 0x7f
    v = 0
    for x in b[i + 1:i + 1 + k]:
        v = (v << 8) | x
    return v, i + 1 + k

def items(b, start, end):
    i = start
    out = []
    while i < end:
        tag = b[i]
        ln, v = read_len(b, i + 1)
        if v + ln > end:
            out.append((tag, i, v, ln, "OVERRUN"))
            break
        out.append((tag, i, v, ln, None))
        i = v + ln
    return out

def nested(b, v, ln):
    try:
        its = items(b, v, v + ln)
        return its and all(x[4] is None for x in its) and its[-1][2] + its[-1][3] == v + ln
    except Exception:
        return False

def dump(b, start, end, depth=0, maxdepth=4, limit=12):
    its = items(b, start, end)
    for n, (tag, i, v, ln, err) in enumerate(its):
        if n >= limit:
            print("  " * depth + f"... {len(its) - limit} more")
            break
        prev = b[v:v + min(ln, 12)].hex(" ")
        print("  " * depth + f"@{i} tag {tag:#04x} len {ln} {err or ''} [{prev}]")
        if depth < maxdepth and ln >= 2 and nested(b, v, ln):
            dump(b, v, v + ln, depth + 1, maxdepth, limit)

if __name__ == "__main__":
    b = open(sys.argv[1], "rb").read()
    dump(b, 0, len(b), maxdepth=int(sys.argv[2]) if len(sys.argv) > 2 else 3)
