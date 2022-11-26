`wizdraw` - tiny no_std crate to fill and stroke bezier curves

### Example

```rust
use vek::bezier::CubicBezier2;
use vek::vec::Vec2;

// these coordinates correspond to pixels
let start = Vec2::new(25.0, 50.0);
let curve1 = CubicBezier2 {
    start,
    ctrl0: Vec2::new(25.0, 25.0),
    ctrl1: Vec2::new(75.0, 25.0),
    end:   Vec2::new(75.0, 50.0),
};
let curve2 = CubicBezier2 {
    start: Vec2::new(75.0, 60.0),
    ctrl0: Vec2::new(75.0, 40.0),
    ctrl1: Vec2::new(25.0, 40.0),
    end:   Vec2::new(25.0, 60.0),
};

let mut points = Vec::new();

// convert the curves to a path;
wizdraw::push_cubic_bezier_segments::<_, 6>(&curve1, 0.2, &mut points);
wizdraw::push_cubic_bezier_segments::<_, 6>(&curve2, 0.2, &mut points);

// close the loop
points.push(start);

// create a buffer to hold the mask
let mask_size = Vec2::new(100, 100);
let mut mask = vec![0u8; mask_size.product()];

// if you want to fill the path:
// (SSAA = 4)
wizdraw::fill::<_, 4>(&points, &mut mask, mask_size);

// or if you'd like to stroke the path:
// (SSAA = 4)
let stroke_width = 2.0;
wizdraw::stroke::<_, 4>(&points, &mut mask, mask_size, stroke_width);
```

### Demo: PNG output

Check out the `png_demo` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/1.1.0/source/output.png)
