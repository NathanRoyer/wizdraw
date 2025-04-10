`wizdraw` - Portable crate to fill and stroke composite bezier curves (paths)

All operations are done on an offscreen canvas.

The CPU implementation is always available and performs OK.

### Limitations

- Pixels are [R, G, B, A] values with 8-bit components
- Point coordinates are pairs of `f32`
- The GLES2 implementation's output is currently only readable as RGBA5551

### Features

- `simd`: include the SIMD canvas implementation
- `gles2`: include the OpenGL ES 2.0 canvas implementation
- `contour`: include path stroking code
- `shapes`: include basic shape generation code

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

let mut canvas = wizdraw::cpu::Canvas::new(500, 500);
canvas.fill_cbc(&oval, &texture, false, SsaaConfig::X4);

// retrieve a framebuffer
let pixels = canvas.pixels();
```

### Demo: PNG output

Check out the `cpu` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/2.2.0/source/output.png)
