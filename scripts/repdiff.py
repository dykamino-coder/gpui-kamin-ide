import os, sys

def rd(p):
    d = {}
    for l in open(p, encoding='utf-8'):
        f = l.rstrip('\n').split('|')
        if len(f) >= 3:
            try:
                d[f[0]] = float(f[2])
            except ValueError:
                d[f[0]] = 99.0
    return d

old, new = rd(sys.argv[1]), rd(sys.argv[2])
reg = [(k, old[k], v) for k, v in new.items() if k in old and old[k] <= 0.5 < v]
fix = [(k, old[k], v) for k, v in new.items() if k in old and v <= 0.5 < old[k]]
print('регрессий', len(reg), 'починено', len(fix))
for tag, rows in (('REG', reg), ('FIX', fix)):
    for k, a, b in rows[:25]:
        print(' ', tag, os.path.basename(k), a, '->', b)
