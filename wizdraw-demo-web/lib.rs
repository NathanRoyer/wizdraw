use wasm_bindgen::{prelude::*, Clamped};
use web_sys::ImageData;
use wizdraw::rgb::AsPixels;
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

    // let origin = Point::new(0.0, 0.0);
    // let size_f32 = size.map(|c| c as f32);

    let top_left =  Point::new(0.600 * wf, 0.100 * hf);
    let top_right = Point::new(x + 100.0, y);
    let btm_left =  Point::new(0.600 * wf, 0.650 * hf);
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

    // let quad = shapes::quad(top_left, top_right, btm_left, btm_right);
    let mut shape = Path::default();
    shape.segment(Segment::new(top_left, top_right));
    shape.segment(Segment::new(top_right, btm_right));
    shape.segment(Segment::new(btm_right, btm_left));
    shape.segment(Segment::new(btm_left, top_left));
    // shape.cubic_bezier([btm_left, btm_right, top_right, top_left], 8.0);

    wiz.clear();
    // wiz.fill(&shape, None, &Texture::Debug, SsaaConfig::None);
    // wiz.fill(&shape, None, &texture, SsaaConfig::None);

    let poly = [
        Point::new(0.12 * wf, 0.1 * hf),
        Point::new(0.4 * wf, 0.15 * hf),
        Point::new(x, y),
        Point::new(0.5 * wf, 0.5 * hf),
        Point::new(0.15 * wf, 0.3 * hf),
    ];

    let quad = wizdraw::shapes::Polygon {
        radius: 5.0,
        points: &poly,
    };

    wiz.fill(&quad, None, &_purple, SsaaConfig::X4);

    /* let offset = -50.0;

    let tl1 = Point::new(x + offset + 25.0, y + offset);
    let tr1 = Point::new(x + offset + 75.0, y + offset);
    let bl1 = Point::new(x + offset + 25.0, y + offset + 100.0);
    let br1 = Point::new(x + offset + 75.0, y + offset + 100.0);

    let tl2 = Point::new(x + offset, y + offset + 25.0);
    let tr2 = Point::new(x + offset + 100.0, y + offset + 25.0);
    let bl2 = Point::new(x + offset, y + offset + 75.0);
    let br2 = Point::new(x + offset + 100.0, y + offset + 75.0);

    let tli = Point::new(x + offset + 25.0, y + offset + 25.0);
    let tri = Point::new(x + offset + 75.0, y + offset + 25.0);
    let bli = Point::new(x + offset + 25.0, y + offset + 75.0);
    let bri = Point::new(x + offset + 75.0, y + offset + 75.0);

    let mut dialog = Path::default();
    dialog.segment(Segment::new(bl2, tl2));
    dialog.segment(Segment::new(tr1, tl1));
    dialog.segment(Segment::new(tr2, br2));
    dialog.segment(Segment::new(bl1, br1));
    let rad = |degrees: f32| degrees.to_radians();
    dialog.arc(Arc::new(tli, 25.0, rad(180.0), rad(90.0), true));
    dialog.arc(Arc::new(tri, 25.0, rad(90.0), rad(0.0), true));
    dialog.arc(Arc::new(bri, 25.0, rad(0.0), rad(-90.0), true));
    dialog.arc(Arc::new(bli, 25.0, rad(-90.0), rad(-180.0), true));

    wiz.fill(&dialog, None, &_purple, SsaaConfig::X4);*/

    if cpu {
        let pixels = state.cpu.pixels();
        let data = Clamped(rgb::bytemuck::cast_slice(pixels));
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(data, w, h)?;

        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        context.put_image_data(&image_data, 0.0, 0.0)?;
    }

    Ok(())
}
