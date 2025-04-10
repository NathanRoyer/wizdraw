use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;
use wizdraw::{rgb::{AsPixels, ComponentSlice}, *};
use std::sync::Mutex;

static WIZDRAW: Mutex<Option<(cpu::Canvas, Option<BitmapHandle>)>> = Mutex::new(None);

#[wasm_bindgen(start)]
fn init() -> Result<(), JsValue> {
    let mut locked = WIZDRAW.lock().unwrap();
    *locked = Some((cpu::Canvas::new(0, 0), None));
    Ok(())
}

const GRID_PNG: &'static [u8] = include_bytes!("../misc/grid.png");

fn read_grid_png() -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(GRID_PNG);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width as _, info.height as _, buf)
}

#[wasm_bindgen]
pub fn frame(x: f32, y: f32) -> Result<(), JsValue> {
    let window = web_sys::window().expect("No window?");

    let w = window.inner_width()?.as_f64().unwrap() as u32;
    let h = window.inner_height()?.as_f64().unwrap() as u32;

    let document = window.document().expect("No document?");
    let canvas = document.get_element_by_id("canvas").expect("No canvas?");
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    canvas.set_width(w);
    canvas.set_height(h);

    let mut locked = WIZDRAW.lock().unwrap();
    let (wizdraw, bitmap) = &mut *locked.as_mut().unwrap();

    let size = wizdraw.framebuffer_size().map(|u| u as u32);

    if size.x != w || size.y != h {
        *wizdraw = cpu::Canvas::new(w as _, h as _);
        let (tex_w, tex_h, tex_p) = read_grid_png();
        *bitmap = Some(wizdraw.alloc_bitmap(tex_w, tex_h));
        wizdraw.fill_bitmap(bitmap.unwrap(), 0, 0, tex_w, tex_h, tex_p.as_pixels());
    }

    let wf = 1000.0;
    let hf = 1000.0;

    let origin = Point::new(0.0, 0.0);
    let size = vek::Vec2::new(w as f32, h as f32);
    let _full_cover = shapes::rectangle(origin, size);

    let top_left =  Point::new(0.400 * wf, 0.400 * hf);
    let top_right = Point::new(x, y);
    let btm_left =  Point::new(0.400 * wf, 0.650 * hf);
    let btm_right = Point::new(x, 0.700 * hf);

    let _purple = Texture::SolidColor(Color::new(127, 0, 200, 255));

    let texture = Texture::Bitmap {
        top_left,
        // top_right,
        // btm_left,
        // btm_right,
        scale: 1.0,
        repeat: true,
        bitmap: bitmap.unwrap(),
    };

    let quad = shapes::quad(top_left, top_right, btm_left, btm_right);
    wizdraw.clear();

    wizdraw.fill_cbc(&quad, &Texture::Debug, SsaaConfig::None);
    wizdraw.fill_cbc(&quad, &texture, SsaaConfig::None);

    let data = Clamped(wizdraw.pixels().as_slice());
    let image_data = ImageData::new_with_u8_clamped_array_and_sh(data, w, h)?;

    let context = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    context.put_image_data(&image_data, 0.0, 0.0)?;

    Ok(())
}
