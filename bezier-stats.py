from random import random
from pprint import pprint
from math import sqrt

def rand100():
    return (random() * 100000) - 50000

class RandomCubicBezier:
    def __init__(self):
        self.start = (rand100(), rand100())
        self.c1    = (rand100(), rand100())
        self.c2    = (rand100(), rand100())
        self.stop  = (rand100(), rand100())

    def compute(self, t):
        u = 1 - t

        a = tuple(c * u * u * u for c in self.start)
        b = tuple(c * u * u * t for c in self.c1)
        c = tuple(c * u * t * t for c in self.c2)
        d = tuple(c * t * t * t for c in self.stop)

        x = sum([t[0] for t in [a, b, c, d]])
        y = sum([t[1] for t in [a, b, c, d]])

        return (x, y)


for seg_p in range(2, 7):
    max_error = 0

    for i in range(100):
        rcb = RandomCubicBezier()

        segments = 2 ** seg_p
        step = 1 / segments
        t1 = 0
        t2 = step
        while t1 < 1:
            x1, y1 = rcb.compute(t1)
            x2, y2 = rcb.compute(t2)

            dx = abs(x2 - x1)
            dy = abs(y2 - y1)

            seg_width = dx + dy

            sub_t = 0
            while sub_t < 1:
                approx_x = x1 + sub_t * dx
                approx_y = y1 + sub_t * dy

                t = t1 + (sub_t * step)
                actual_x, actual_y = rcb.compute(t)

                error_x = abs(actual_x - approx_x)
                error_y = abs(actual_y - approx_y)
                error = (error_x + error_y) / seg_width

                if error > max_error and seg_width > 5:
                    max_error = error

                sub_t += step

            t1 += step
            t2 += step

    print(str(segments) + " segments: " + str(max_error))
