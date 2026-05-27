`wizdraw` - Portable crate to fill (and stroke) paths

The CPU implementation is always available and performs OK.
There is also an OpenGL ES 2.0 implementation.

### Limitations

- Pixels are [R, G, B, A] values with 8-bit components
- Point coordinates are pairs of `f32`
- The GLES2 implementation's output is currently only readable as RGBA5551

### Features

- `gles2`: include the OpenGL ES 2.0 canvas implementation
- `shapes`: include basic shape generation code
- `text`: include support for text/glyph rendering using ttf-parser

### Example

```rust
use wizdraw::{Arc, Path, Color, Texture, Point, SsaaConfig, PathApi, Canvas};

let myrtle = Color::new(255, 100, 100, 255);
let texture = Texture::SolidColor(myrtle);

let center = Point::new(250.0, 250.0);

let mut path = wizdraw::Path::default();
path.arc(Arc::disk(center, 150.0));

let mut canvas = wizdraw::cpu::Canvas::new(500, 500);
canvas.draw(&path, &texture, SsaaConfig::X4);

// retrieve a framebuffer
let pixels = canvas.pixels();
```

### Demo: PNG output

Check out the `cpu` example to generate this image:

![output.png](https://docs.rs/crate/wizdraw/2.3.0/source/output.png)
