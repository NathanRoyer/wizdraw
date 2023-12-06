use rgb::ComponentBytes;
use vek::vec::Vec2;
use wizdraw::{CubicBezier, util, SsaaConfig};
use std::time::Instant;

fn main() {
    let w = 1000;
    let h = 300;

    let wf = w as f32;
    let hf = h as f32;

    let path = [
        CubicBezier {
            c1: Vec2::new(0.250 * wf, 0.500 * hf),
            c2: Vec2::new(0.250 * wf, 0.100 * hf),
            c3: Vec2::new(0.750 * wf, 0.100 * hf),
            c4: Vec2::new(0.750 * wf, 0.500 * hf),
        },
        CubicBezier {
            c1: Vec2::new(0.750 * wf, 0.500 * hf),
            c2: Vec2::new(0.750 * wf, 0.900 * hf),
            c3: Vec2::new(0.250 * wf, 0.900 * hf),
            c4: Vec2::new(0.250 * wf, 0.500 * hf),
        },
    ];

    let mut line = Vec::new();
    util::stroke_path(path.as_slice(), 5.0, &mut line, 1.0);
    let myrtle = |_x, _y| wizdraw::Color::new(100, 100, 255, 255);

    let mut canvas = wizdraw::Canvas::new(w, h);

    if false {
        let configs = [SsaaConfig::None, SsaaConfig::X2, SsaaConfig::X4, SsaaConfig::X8, SsaaConfig::X16];
        for (simd, dbg) in [(false, "SEQ"), (true, "SIMD")] {
            for config in configs {
                let shots = 10;
                let before = Instant::now();

                for _ in 0..shots {
                    canvas.clear();
                    canvas.fill(&line, util::rainbow, simd, config, false);
                }

                let elapsed = before.elapsed().as_millis() as f32;
                let avg_frame_time = elapsed / (shots as f32);

                print!("[{}, {:?}] ", dbg, config);
                println!("frame time: {}ms = FPS {}", avg_frame_time.round(), (1000.0 / avg_frame_time).round());
            }
        }
    }

    canvas.fill(&path, util::rainbow, true, SsaaConfig::X16, false);
    canvas.fill(&line, myrtle, true, SsaaConfig::X16, false);

    // converting the mask to a PNG image

    use std::fs::File;
    use std::io::BufWriter;

    let pixels = canvas.pixels().as_bytes();

    let file = File::create("output.png").unwrap();
    let ref mut writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, w as _, h as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
}

