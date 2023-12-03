`wizdraw` - tiny no_std crate to fill and stroke bezier curves (partially SIMD)

### Features

- `f64`: use `f64`s instead of `f32`s
- `simd`: use the SIMD version of the `fill` function

By default, this crate uses `f32` and doesn't use SIMD.

### Example

```oeruh
use vek::bezier::CubicBezier2;
use vek::vec::Vec2;

// these coordinates correspond to pixels
let start = Vec2::new(250.0, 500.0);
let curve1 = CubicBezier2 {
    start,
    ctrl0: Vec2::new(250.0, 250.0),
    ctrl1: Vec2::new(750.0, 250.0),
    end:   Vec2::new(750.0, 500.0),
};
let curve2 = CubicBezier2 {
    start: Vec2::new(750.0, 600.0),
    ctrl0: Vec2::new(750.0, 400.0),
    ctrl1: Vec2::new(250.0, 400.0),
    end:   Vec2::new(250.0, 600.0),
};

let mut points = Vec::new();

// convert the curves to a path;
wizdraw::push_cubic_bezier_segments::<6>(&curve1, 0.2, &mut points);
wizdraw::push_cubic_bezier_segments::<6>(&curve2, 0.2, &mut points);

// close the loop
points.push(start);

// create a buffer to hold the mask
let mask_size = Vec2::new(1000, 1000);
let mut mask = vec![0u8; mask_size.product()];

// if you want to fill the path:
// (SSAA = 4, squared = 16)
wizdraw::fill::<4, 16>(&points, &mut mask, mask_size);

// or if you'd like to stroke the path:
// (SSAA = 4)
let stroke_width = 2.0;
wizdraw::stroke::<4>(&points, &mut mask, mask_size, stroke_width);
```

### Demo: PNG output

Check out the `png_demo` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/1.1.0/source/output.png)
