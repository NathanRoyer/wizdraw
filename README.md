`wizdraw` - Tiny no_std crate to fill and stroke composite bezier curves (SIMD/SSAA)

### Limitations

- Pixels are [R, G, B, A] values with 8-bit components
- Point coordinates are pairs of `f32`

### Features

- `simd`: include SIMD code, which can speed rendering up when anti-aliasing is used.
- `stroke`: include the `util::stroke_path` utility function.

By default, this crate doesn't use SIMD because a nightly toolchain is required for that.

### Repository Information

- `wizdraw`: the main crate
- `wizdraw-demo-web`: demo of wizdraw on the web using wasm-bindgen
