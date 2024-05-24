`wizdraw` - Tiny no_std crate to fill and stroke composite bezier curves (SIMD/SSAA)

### Limitations

- Pixels are [R, G, B, A] values with 8-bit components
- Point coordinates are pairs of `f32`

### Features

- `simd`: include SIMD code, which can speed rendering up when anti-aliasing is used.
- `contour`: include the [`contour()`] utility function, which allows you to stroke paths.

By default, this crate doesn't use SIMD because a nightly toolchain is required for that.

### Example

```rust
let myrtle = Color::new(255, 100, 100, 255);
let texture = Texture::SolidColor(myrtle);

let oval = [
    CubicBezier {
        c1: Point::new(50.0, 250.0),
        c2: Point::new(50.0, 50.0),
        c3: Point::new(450.0, 50.0),
        c4: Point::new(450.0, 250.0),
    },
    CubicBezier {
        c1: Point::new(450.0, 250.0),
        c2: Point::new(450.0, 450.0),
        c3: Point::new(50.0, 450.0),
        c4: Point::new(50.0, 250.0),
    },
];

let mut canvas = wizdraw::cpu::Canvas::new_seq(500, 500);
canvas.fill_cbc(&oval, &texture, false, SsaaConfig::X4);
```

### Demo: PNG output

Check out the `png_demo` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/2.1.0/source/output.png)
