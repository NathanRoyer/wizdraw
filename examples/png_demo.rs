fn main() {
    use vek::bezier::CubicBezier2;
    use vek::vec::Vec2;

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

    wizdraw::push_cubic_bezier_segments::<6>(&curve1, 0.2, &mut points);
    wizdraw::push_cubic_bezier_segments::<6>(&curve2, 0.2, &mut points);

    // close the loop
    points.push(start);

    let mask_size = Vec2::new(1000, 1000);
    let mut mask = vec![0u8; mask_size.product()];
    let stroke_width = 20.0;

    wizdraw::fill::<4, 16>(&points, &mut mask, mask_size);

    // converting the mask to a PNG image

    use std::fs::File;
    use std::io::BufWriter;

    let mut pixels = Vec::new();

    for opacity in &mask {
        pixels.push(*opacity);
        pixels.push(*opacity);
        pixels.push(*opacity);
        pixels.push(255);
    }

    wizdraw::stroke::<4>(&points, &mut mask, mask_size, stroke_width);

    for i in 0..mask.len() {
        let opacity = mask[i] as f32 / 255.0;

        pixels[i * 4 + 0] = (255.0 * opacity + ((1.0 - opacity) * pixels[i * 4 + 0] as f32)) as u8;
        pixels[i * 4 + 1] = (  0.0 * opacity + ((1.0 - opacity) * pixels[i * 4 + 1] as f32)) as u8;
        pixels[i * 4 + 2] = (  0.0 * opacity + ((1.0 - opacity) * pixels[i * 4 + 2] as f32)) as u8;
    }

    let file = File::create("output.png").unwrap();
    let ref mut w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, mask_size.x as _, mask_size.y as _);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&pixels).unwrap();
}

