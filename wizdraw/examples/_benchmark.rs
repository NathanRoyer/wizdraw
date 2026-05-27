use rgb::AsPixels;
use wizdraw::{Canvas, Color, Texture, Point, Path, Shape, SsaaConfig};

use std::time::Instant;

const GRID_PNG: &'static [u8] = include_bytes!("../../misc/grid.png");
const NOTO_TTF: &'static [u8] = include_bytes!("../../misc/noto-sans.ttf");

fn read_grid_png() -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(GRID_PNG);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width as _, info.height as _, buf)
}

pub fn benchmark<C: Canvas>(canvas: &mut C, wf: f32, hf: f32) {
    let mut path = Path::default();
    let max_error = 2.0;

    path.cubic_bezier([
        Point::new(0.250 * wf, 0.500 * hf),
        Point::new(0.250 * wf, 0.100 * hf),
        Point::new(0.750 * wf, 0.100 * hf),
        Point::new(0.750 * wf, 0.500 * hf),
    ], max_error);

    path.cubic_bezier([
        Point::new(0.750 * wf, 0.500 * hf),
        Point::new(0.750 * wf, 0.900 * hf),
        Point::new(0.250 * wf, 0.900 * hf),
        Point::new(0.250 * wf, 0.500 * hf),
    ], max_error);

    let mut font = wizdraw::text::Font::new(NOTO_TTF);
    let font_size = 200.0;
    let text_width = font.text_width("Hello World!", font_size);
    let text_xy = Point::new(0.5 * wf - 0.5 * text_width, 0.5 * hf - font_size * 0.5);
    let text_shape = font.text_shape("Hello World!", font_size, text_xy);

    let (tex_w, tex_h, tex_p) = read_grid_png();

    let green = Color::new(100, 200, 150, 255);
    let green = Texture::SolidColor(green);

    let white = Color::new(255, 255, 255, 255);
    let white = Texture::SolidColor(white);

    for ssaa in [SsaaConfig::None, SsaaConfig::X2, SsaaConfig::X4] {
        canvas.clear();

        let bitmap = canvas.alloc_bitmap(tex_w, tex_h);
        canvas.fill_bitmap(bitmap, 0, 0, tex_w, tex_h, tex_p.as_pixels());

        let texture = Texture::QuadBitmap {
            top_left:  Point::new(0.410 * wf, 0.300 * hf),
            top_right: Point::new(0.590 * wf, 0.370 * hf),
            btm_left:  Point::new(0.410 * wf, 0.700 * hf),
            btm_right: Point::new(0.590 * wf, 0.630 * hf),
            bitmap,
        };

        if true {
            let then = Instant::now();
            let num = 20;
            for _ in 0..num {
                canvas.draw(&path, &Texture::Debug, ssaa);
                canvas.draw(&path, &texture, ssaa);

                let path = path.strike(4.0);
                canvas.draw(&path, &green, ssaa);

                canvas.draw(&text_shape, &white, ssaa);
            }
            let avg_ms = then.elapsed().as_micros() / num;
            let fps = 1000000 / avg_ms;
            println!("{:?}: {}us = {} FPS", ssaa, avg_ms, fps);
        }
    }
}