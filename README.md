`wizdraw` - Tiny no_std crate to fill and stroke composite bezier curves (SIMD/SSAA)

### Limitations

- Pixels are [R, G, B, A] values with 8-bit components
- Point coordinates are pairs of `f32`

### Features

- `simd`: include SIMD code, which can speed rendering up when anti-aliasing is used.
- `stroke`: include the `util::stroke_path` utility function.

By default, this crate doesn't use SIMD because a nightly toolchain is required for that.

### Example

```rust
use wizdraw::{Canvas, Color, CubicBezier, Point, SsaaConfig, util};
use wizdraw::rgb::ComponentBytes;

// size of our output buffer: 1000x1000px
let mut canvas = Canvas::new(1000, 1000);

// the unit for these coordinates is a pixel
let path = [
    CubicBezier {
        c1: Point::new(250.0, 600.0),
        c2: Point::new(250.0, 250.0),
        c3: Point::new(750.0, 250.0),
        c4: Point::new(750.0, 600.0),
    },
    CubicBezier {
        c1: Point::new(750.0, 600.0),
        c2: Point::new(750.0, 400.0),
        c3: Point::new(250.0, 400.0),
        c4: Point::new(250.0, 600.0),
    },
];

// We'll generate another path which represents a line along the other path
let mut contour = Vec::new();
let stroke_width = 10.0; // px
let max_error = 1.0; // px
util::stroke_path(&path, stroke_width, &mut contour, max_error);

// use SIMD functions, if this wizdraw build has the feature
let try_to_use_simd = true;

// we'll use the highest SSAA config
let ssaa = SsaaConfig::X16;

// path holes won't show up on the output
let dont_show_holes = false;

// fill the path with a rainbow texture
canvas.fill(&path, util::rainbow, try_to_use_simd, ssaa, dont_show_holes);

// we'll draw the contour in myrtle
let myrtle = |_x, _y| Color::new(100, 100, 255, 255);
canvas.fill(&contour, myrtle, try_to_use_simd, ssaa, dont_show_holes);

// Time to use our render!
let pixels: &[Color] = canvas.pixels();
let bytes: &[u8] = pixels.as_bytes();
```

### Demo: PNG output

Check out the `png_demo` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/2.0.0/source/output.png)
