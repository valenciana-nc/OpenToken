"""Generate the original OpenToken app artwork, without external imaging dependencies."""
import math, struct, zlib
from pathlib import Path
out = Path(__file__).resolve().parents[1] / 'src-tauri' / 'icons'
out.mkdir(exist_ok=True)
def png(size):
    rows=[]
    for y in range(size):
        row=bytearray()
        for x in range(size):
            u,v=(x+.5)/size,(y+.5)/size
            dx,dy=abs(u-.5),abs(v-.5)
            inside=max(dx-.31,0)**2+max(dy-.31,0)**2 < .17**2
            color=(83,105,216,255) if inside else (0,0,0,0)
            # Three offset token layers in the center of a violet rounded tile.
            for offset in [.15,0,-.15]:
                diamond=abs(u-.5)/.27+abs(v-.5-offset)/.115
                if .73 < diamond < 1.04 and .15<u<.85:
                    color=(255,255,255,255)
            row.extend(color)
        rows.append(b'\0'+row)
    def chunk(t,d): return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
    return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',size,size,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows),9))+chunk(b'IEND',b'')
for size in [32,128,256]: (out/f'{size}x{size}.png').write_bytes(png(size))
data=png(256)
(out/'icon.ico').write_bytes(struct.pack('<HHH',0,1,1)+struct.pack('<BBBBHHII',0,0,0,0,1,32,len(data),22)+data)
(out/'icon.png').write_bytes(png(128))
