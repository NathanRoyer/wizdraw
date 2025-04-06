use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;
use wizdraw::rgb::{AsPixels, ComponentSlice};
use wizdraw::vek::Vec2;
use wizdraw::*;
use std::sync::Mutex;

static WIZDRAW: Mutex<Option<State>> = Mutex::new(None);
const GRID_PNG: &'static [u8] = include_bytes!("../misc/grid.png");

unsafe impl Send for State {}

struct State {
    cpu: cpu::Canvas,
    cpu_bmp: BitmapHandle,
    gles2: gles2::Canvas,
    gles2_bmp: BitmapHandle,
    size: Vec2<usize>,
}

#[wasm_bindgen(start)]
fn init() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

fn init_state(size: Vec2<usize>) -> State {
    use wasm_bindgen::JsCast;

    let mut cpu_canvas = cpu::Canvas::new(size.x, size.y);

    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("gpu-canvas").unwrap().dyn_into::<web_sys::HtmlCanvasElement>().unwrap();
    let webgl_context = canvas.get_context("webgl").unwrap().unwrap().dyn_into::<web_sys::WebGlRenderingContext>().unwrap();
    let gl = glow::Context::from_webgl1_context(webgl_context);
    let mut gles2_canvas = gles2::Canvas::init(gl, size.x as i32, size.y as i32, false).unwrap();

    let cpu_bmp = load_grid(&mut cpu_canvas);
    let gles2_bmp = load_grid(&mut gles2_canvas);

    State {
        cpu: cpu_canvas,
        cpu_bmp,
        gles2: gles2_canvas,
        gles2_bmp,
        size,
    }
}

fn load_grid<C: wizdraw::Canvas>(canvas: &mut C) -> BitmapHandle {
    let (tex_w, tex_h, tex_p) = read_grid_png();
    let bmp_id = canvas.alloc_bitmap(tex_w, tex_h);
    canvas.fill_bitmap(bmp_id, 0, 0, tex_w, tex_h, tex_p.as_pixels());
    bmp_id
}

fn read_grid_png() -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(GRID_PNG);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width as _, info.height as _, buf)
}

#[wasm_bindgen]
pub fn frame(x: f32, y: f32, cpu: bool) -> Result<(), JsValue> {
    let window = web_sys::window().expect("No window?");

    let w = window.inner_width()?.as_f64().unwrap() as u32;
    let h = window.inner_height()?.as_f64().unwrap() as u32;

    let canvas_id = match cpu {
        true => "cpu-canvas",
        false => "gpu-canvas",
    };

    let document = window.document().expect("No document?");
    let canvas = document.get_element_by_id(canvas_id).expect("No canvas?");
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    canvas.set_width(w);
    canvas.set_height(h);

    let size = Vec2::new(w as usize, h as usize);

    let mut locked = WIZDRAW.lock().unwrap();
    let maybe_state = &mut *locked;

    let reset = match maybe_state {
        Some(state) => state.size != size,
        None => true,
    };

    if reset {
        *locked = Some(init_state(size));
    }

    let state = locked.as_mut().unwrap();

    let wf = 1000.0;
    let hf = 1000.0;

    let origin = Point::new(0.0, 0.0);
    let size_f32 = size.map(|c| c as f32);
    let _full_cover = shapes::rectangle(origin, size_f32);

    let top_left =  Point::new(0.400 * wf, 0.400 * hf);
    let top_right = Point::new(x, y);
    let btm_left =  Point::new(0.400 * wf, 0.650 * hf);
    let btm_right = Point::new(x, 0.700 * hf);

    let _purple = Texture::SolidColor(Color::new(127, 0, 200, 255));

    let (wiz, bitmap): (&mut dyn Canvas, _) = match cpu {
        true => (&mut state.cpu, state.cpu_bmp),
        false => (&mut state.gles2, state.gles2_bmp),
    };

    let texture = Texture::Bitmap {
        top_left,
        // top_right,
        // btm_left,
        // btm_right,
        scale: 1.0,
        repeat: true,
        bitmap,
    };

    let quad = shapes::quad(top_left, top_right, btm_left, btm_right);
    wiz.clear();

    wiz.fill_cbc(&quad, &Texture::Debug, SsaaConfig::None);
    wiz.fill_cbc(&quad, &texture, SsaaConfig::None);

    if cpu {
        let data = Clamped(state.cpu.pixels().as_slice());
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(data, w, h)?;

        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        context.put_image_data(&image_data, 0.0, 0.0)?;
    }

    Ok(())
}
