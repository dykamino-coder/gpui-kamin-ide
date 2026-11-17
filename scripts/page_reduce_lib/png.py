"""PNG RGB/RGBA8 decoding and rectangular comparison, using only stdlib."""

import struct
import zlib


def read(path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    offset, compressed, header, ended = 8, bytearray(), None, False
    while offset + 12 <= len(data):
        size = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + size]
        crc = data[offset + 8 + size:offset + 12 + size]
        if len(crc) != 4 or zlib.crc32(kind + payload) != struct.unpack(">I", crc)[0]:
            raise ValueError("truncated/corrupt PNG")
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            compressed.extend(payload)
        offset += size + 12
        if kind == b"IEND":
            ended = True
            break
    if header is None or not ended:
        raise ValueError("missing PNG header or end")
    width, height, bits, color, compression, filtering, interlace = header
    if bits != 8 or color not in (2, 6) or compression or filtering or interlace:
        raise ValueError("only noninterlaced RGB/RGBA8 PNG is supported")
    channels = 3 if color == 2 else 4
    stride = width * channels
    raw = zlib.decompress(compressed)
    if len(raw) != height * (stride + 1):
        raise ValueError("invalid PNG scanline size")
    rows, previous = [], bytearray(stride)
    for y in range(height):
        base = y * (stride + 1)
        mode = raw[base]
        row = bytearray(raw[base + 1:base + 1 + stride])
        if mode > 4:
            raise ValueError("invalid PNG filter")
        # Page captures mostly use Up rows. Handle that filter without the
        # per-byte predictor dispatch used by the other PNG filter modes.
        if mode == 2:
            row = bytearray((value + above) & 255 for value, above in zip(row, previous))
        for x in range(stride) if mode not in (0, 2) else ():
            a = row[x - channels] if x >= channels else 0
            b = previous[x]
            c = previous[x - channels] if x >= channels else 0
            if mode == 1:
                value = a
            elif mode == 3:
                value = (a + b) // 2
            elif mode == 4:
                p = a + b - c
                distances = (abs(p - a), abs(p - b), abs(p - c))
                value = (a, b, c)[distances.index(min(distances))]
            else:
                value = 0
            row[x] = (row[x] + value) & 255
        # Composite transparency onto white, as a page canvas does.
        if channels == 3:
            rgb = row
        elif row[3::4] == b"\xff" * width:
            rgb = bytearray(width * 3)
            for channel in range(3):
                rgb[channel::3] = row[channel::4]
        else:
            rgb = bytearray()
            for x in range(0, stride, channels):
                alpha = row[x + 3]
                rgb.extend((v * alpha + 255 * (255 - alpha) + 127) // 255 for v in row[x:x + 3])
        rows.append(rgb)
        previous = row
    return width, height, rows


def different(left, right, rect, tolerance=0):
    lw, lh, lrows = read(left)
    rw, rh, rrows = read(right)
    if (lw, lh) != (rw, rh):
        raise ValueError(f"screenshot sizes differ: {lw}x{lh} vs {rw}x{rh}")
    x, y, width, height = rect
    if min(x, y) < 0 or min(width, height) <= 0 or x + width > lw or y + height > lh:
        raise ValueError("rectangle is outside the screenshots")
    return any(abs(a - b) > tolerance for row in range(y, y + height)
               for a, b in zip(lrows[row][3*x:3*(x + width)], rrows[row][3*x:3*(x + width)]))
