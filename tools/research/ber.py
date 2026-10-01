"""BER-style tree dump with multi-byte tags (for the expanded properties block)."""
import sys

def tag_at(b, i):
    t0 = b[i]; i += 1
    num = t0 & 0x1f
    if num == 0x1f:
        num = 0
        while True:
            x = b[i]; i += 1
            num = (num << 7) | (x & 0x7f)
            if not x & 0x80:
                break
    cls = "UACP"[t0 >> 6]
    cons = bool(t0 & 0x20)
    return (cls, cons, num, t0), i

def len_at(b, i):
    n = b[i]; i += 1
    if n < 0x80:
        return n, i
    k = n & 0x7f
    v = 0
    for _ in range(k):
        v = (v << 8) | b[i]; i += 1
    return v, i

def walk(b, s, e, d=0, out=print):
    i = s
    while i < e:
        (cls, cons, num, t0), j = tag_at(b, i)
        ln, v = len_at(b, j)
        val = b[v:v + ln]
        txt = ""
        if not cons:
            try:
                if ln > 2 and val[-1] == 0 and all(32 <= c < 127 for c in val[:-1]):
                    txt = repr(val[:-1].decode())
            except Exception:
                pass
            if not txt and ln >= 4 and ln % 2 == 0 and all(val[k + 1] == 0 for k in range(0, ln, 2)):
                txt = repr(val.decode("utf-16-le", "replace"))
        out("  " * d + f"{cls}{'*' if cons else ''}{num} ({t0:02x}) len {ln} {val[:24].hex(' ') if not cons else ''} {txt}")
        if cons:
            walk(b, v, v + ln, d + 1, out)
        i = v + ln

if __name__ == "__main__":
    b = open(sys.argv[1], "rb").read()
    walk(b, 0, len(b))
