use super::*;

use ttf_parser::{Face, OutlineBuilder};
use litemap::LiteMap;

const BASE_SIZE: f32 = 200.0;
const MAX_ERROR: f32 = 4.0;

pub struct GlyphData {
    pub path: Path,
    pub h_bearing: f32,
    pub h_advance: f32,
}

pub struct Font<T: AsRef<[u8]>> {
    bytes: T,
    cache: LiteMap<char, GlyphData>,
}

fn glyph_path(bytes: &[u8], glyph: char) -> Option<GlyphData> {
    let face = Face::parse(bytes, 0).ok()?;
    let index = face.glyph_index(glyph)?;

    let height = face.height() as f32;
    let ascender = face.ascender() as f32;
    let scaler = height / BASE_SIZE;

    if height == 0.0 {
        return None;
    }

    let h_advance = face.glyph_hor_advance(index).unwrap_or(scaler as u16);
    let h_bearing = face.glyph_hor_side_bearing(index).unwrap_or(0);
    let h_advance = (h_advance as f32) / scaler;
    let h_bearing = (h_bearing as f32) / scaler;

    let mut path = GlyphPath {
        inner: Path::default(),
        cursor: None,
        start: None,
        ascender,
        scaler,
    };

    let _rect = face.outline_glyph(index, &mut path)?;

    let data = GlyphData {
        path: path.inner,
        h_bearing,
        h_advance,
    };

    Some(data)
}

impl<T: AsRef<[u8]>> Font<T> {
    pub fn new(bytes: T) -> Self {
        let mut cache = LiteMap::new();
        cache.insert(' ', whitespace_path());

        Self {
            bytes,
            cache,
        }
    }

    pub fn get_preloaded(&self, glyph: char) -> &GlyphData {
        self.cache.get(&glyph).expect("glyph was not preloaded")
    }

    pub fn preload(&mut self, glyph: char) {
        if !self.cache.contains_key(&glyph) {
            let bytes = self.bytes.as_ref();

            let data = match glyph_path(bytes, glyph) {
                Some(data) => data,
                None => fallback_path(),
            };

            self.cache.insert(glyph, data);
        }
    }

    pub fn preload_all(&mut self, text: &str) {
        for glyph in text.chars() {
            self.preload(glyph);
        }
    }

    pub fn get(&mut self, glyph: char) -> &GlyphData {
        self.preload(glyph);
        self.get_preloaded(glyph)
    }

    pub fn text_width(&mut self, text: &str, size: f32) -> f32 {
        let scaler = size / BASE_SIZE;
        let mut total = 0.0;

        for glyph in text.chars() {
            let data = self.get(glyph);
            total += data.h_advance * scaler;
            // todo: h_bearing
        }

        total
    }

    pub fn text_shape<'a>(&'a mut self, text: &'a str, size: f32, top_left: Point) -> TextShape<'a, T> {
        // ensure all glyphs are loaded, allowing self to become an immutable referenec
        self.preload_all(text);

        TextShape {
            font: self,
            top_left,
            text,
            scaler: size / BASE_SIZE,
        }
    }
}

#[derive(Default)]
struct GlyphPath {
    inner: Path,
    start: Option<Point>,
    cursor: Option<Point>,
    ascender: f32,
    scaler: f32,
}

impl GlyphPath {
    fn adjusted(&self, x: f32, y: f32) -> Point {
        Point::new(x, self.ascender - y) / self.scaler
    }

    fn cubic_bezier(&mut self, points: [Point; 4]) {
        self.inner.cubic_bezier(points, MAX_ERROR);
        // self.inner.segment((points[0], points[3]).into());
        self.cursor = Some(points[3]);
    }
}

impl OutlineBuilder for GlyphPath {
    fn move_to(&mut self, x: f32, y: f32) {
        let point = self.adjusted(x, y);
        self.start = Some(point);
        self.cursor = Some(point);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let a = self.cursor.expect("no cursor!");
        let b = self.adjusted(x, y);
        self.inner.segment((a, b).into());
        self.cursor = Some(b);
    }

    fn quad_to(&mut self, x2: f32, y2: f32, x3: f32, y3: f32) {
        let p1 = self.cursor.expect("no cursor!");
        let ctrl = self.adjusted(x2, y2);
        let p4 = self.adjusted(x3, y3);
        let p2 = (p1 + ctrl + ctrl) / 3.0;
        let p3 = (p4 + ctrl + ctrl) / 3.0;
        self.cubic_bezier([p1, p2, p3, p4]);
    }

    fn curve_to(&mut self, x2: f32, y2: f32, x3: f32, y3: f32, x4: f32, y4: f32) {
        let p1 = self.cursor.expect("no cursor!");
        let p2 = self.adjusted(x2, y2);
        let p3 = self.adjusted(x3, y3);
        let p4 = self.adjusted(x4, y4);
        self.cubic_bezier([p1, p2, p3, p4]);
    }

    fn close(&mut self) {
        /*if let Some(start) = self.start {
            self.line_to(start.x, start.y);
        }*/
    }
}

pub struct TextShape<'a, T: AsRef<[u8]>> {
    font: &'a mut Font<T>,
    top_left: Point,
    text: &'a str,
    scaler: f32,
}

impl<'a, T: AsRef<[u8]>> Shape for TextShape<'a, T> {
    fn draw(&self, api: &mut dyn PathApi) {
        let mut translation = self.top_left;
        for glyph in self.text.chars() {
            let data = self.font.get_preloaded(glyph);
            data.path.transform(self.scaler, translation).draw(api);
            translation.x += data.h_advance;
        }
    }
}

fn fallback_path() -> GlyphData {
    // todo: hollow rectangle
    let mut path = Path::default();
    let h_advance = BASE_SIZE * 0.6;
    let h_bearing = BASE_SIZE * 0.05;
    let v_bearing = BASE_SIZE * 0.05;
    let thickness = BASE_SIZE * 0.025;

    let elx = h_bearing;
    let erx = h_advance - h_bearing;
    let ilx = elx + thickness;
    let irx = erx - thickness;

    let ety = v_bearing;
    let eby = BASE_SIZE - v_bearing;
    let ity = ety + thickness;
    let iby = eby - thickness;

    let etl = Point::new(elx, ety);
    let etr = Point::new(erx, ety);
    let ebl = Point::new(elx, eby);
    let ebr = Point::new(erx, eby);

    let itl = Point::new(ilx, ity);
    let itr = Point::new(irx, ity);
    let ibl = Point::new(ilx, iby);
    let ibr = Point::new(irx, iby);

    path.segment((etl, etr).into());
    path.segment((etr, ebr).into());
    path.segment((ebr, ebl).into());
    path.segment((ebl, etl).into());
    path.segment((itl, itr).into());
    path.segment((itr, ibr).into());
    path.segment((ibr, ibl).into());
    path.segment((ibl, itl).into());
    
    GlyphData {
        path,
        h_bearing,
        h_advance,
    }
}

fn whitespace_path() -> GlyphData {
    let h_advance = BASE_SIZE * 0.3;
    let h_bearing = h_advance / 2.0;

    GlyphData {
        path: Path::default(),
        h_bearing,
        h_advance,
    }
}
