use super::*;

use core::cell::Cell;
use core::mem::swap;

/////////////////////////////////////////////////////////

const CURVATURE_INIT_0: u8 = 0;
const CURVATURE_INIT_1: u8 = 1;
const CURVATURE_SFSG: u8 = 2;
const CURVATURE_BAD: u8 = 255;

#[derive(Copy, Clone, Default)]
struct CurvatureMonitor {
    curvature_state: u8,
    first_point: Point,
    first_proj: Point,
    prev_point: Point,
    prev_proj: Point,
}

impl CurvatureMonitor {
    fn is_live(&self) -> bool {
        self.curvature_state != CURVATURE_BAD
    }

    fn segment(&mut self, segment: &Segment) {
        if self.curvature_state == CURVATURE_INIT_0 {
            // println!("segment init: {}, {}", segment.a, segment.b);
            self.curvature_state = CURVATURE_INIT_1;
            self.first_point = segment.a;
            self.first_proj = segment.b;
            self.prev_point = segment.b;
            self.prev_proj = segment.a;
            return;
        }

        let is_prev = |prev: Point, p: Point| (prev - p).magnitude_squared() < 0.001;

        let mut a_is_prev = is_prev(self.prev_point, segment.a);
        let mut b_is_prev = is_prev(self.prev_point, segment.b);

        if self.curvature_state == CURVATURE_INIT_1 {
            if !(a_is_prev | b_is_prev) {
                swap(&mut self.first_point, &mut self.prev_point);
                swap(&mut self.first_proj, &mut self.prev_proj);
                a_is_prev = is_prev(self.prev_point, segment.a);
                b_is_prev = is_prev(self.prev_point, segment.b);
            }

            self.curvature_state = CURVATURE_SFSG;
        }

        if !(a_is_prev | b_is_prev) {
            // println!("segment bad: {}, {}, {}", segment.a, segment.b, self.prev_point);
            self.curvature_state = CURVATURE_BAD;
            return;
        }

        if !aligned([segment.a, segment.b, self.prev_proj]) {
            // println!("segment mis: {}, {}, {}", segment.a, segment.b, self.prev_proj);
            self.curvature_state = CURVATURE_BAD;
            return;
        }

        let (prev_point, prev_proj) = match a_is_prev {
            true => (segment.b, segment.a),
            false => (segment.a, segment.b),
        };

        self.prev_proj = prev_proj;
        self.prev_point = prev_point;
        // println!("segment good: {}, {}", segment.a, segment.b);
    }

    fn arc(&mut self, arc: &Arc) {
        let [start, stop] = arc.ends();

        if self.curvature_state == CURVATURE_INIT_0 {
            // println!("arc init: {}, {}", start, stop);
            let start_dir = rotate_90deg(start - arc.center);
            let stop_dir = rotate_90deg(stop - arc.center);
            let start_proj = start + start_dir;
            let stop_proj = stop + stop_dir;

            self.curvature_state = CURVATURE_INIT_1;
            self.first_point = start;
            self.first_proj = start_proj;
            self.prev_point = stop;
            self.prev_proj = stop_proj;
            return;
        }

        let is_prev = |prev: Point, p: Point| (prev - p).magnitude_squared() < 0.001;

        let mut start_is_prev = is_prev(self.prev_point, start);
        let mut stop_is_prev = is_prev(self.prev_point, stop);

        if self.curvature_state == CURVATURE_INIT_1 {
            if !(start_is_prev | stop_is_prev) {
                swap(&mut self.first_point, &mut self.prev_point);
                swap(&mut self.first_proj, &mut self.prev_proj);
                start_is_prev = is_prev(self.prev_point, start);
                stop_is_prev = is_prev(self.prev_point, stop);
            }

            self.curvature_state = CURVATURE_SFSG;
        }

        if !(start_is_prev | stop_is_prev) {
            // println!("arc bad: {}, {}, {}", start, stop, self.prev_point);
            self.curvature_state = CURVATURE_BAD;
            return;
        }

        let (this, next) = match start_is_prev {
            true => (start, stop),
            false => (stop, start),
        };

        let this_dir = rotate_90deg(this - arc.center);
        let next_dir = rotate_90deg(next - arc.center);

        let this_proj = this + this_dir;
        let next_proj = next + next_dir;

        if !aligned([this_proj, this, self.prev_proj]) {
            // println!("arc mis: {}, {}, {}", start, stop, self.prev_proj);
            self.curvature_state = CURVATURE_BAD;
            return;
        }

        self.prev_proj = next_proj;
        self.prev_point = next;
        // println!("arc good: {}, {}", start, stop);
    }

    fn finalize(&self) -> bool {
        let sfsg = self.curvature_state == CURVATURE_SFSG;
        let looped = self.prev_point == self.first_point;
        let points = [self.prev_proj, self.prev_point, self.first_proj];
        sfsg && looped && aligned(points)
    }
}

struct InitialStriker<'a> {
    shape: &'a dyn Shape,
    width: f32,

    cm: &'a Cell<CurvatureMonitor>,
}

struct StrikerApi<'a> {
    inner: &'a InitialStriker<'a>,
    real_api: &'a mut dyn PathApi,
}

impl PathApi for StrikerApi<'_> {
    fn segment(&mut self, mut segment: Segment) {
        if segment.a == segment.b {
            return;
        }

        let mut cm = self.inner.cm.get();
        if cm.is_live() {
            cm.segment(&segment);
            self.inner.cm.set(cm);
        }

        let a_to_b = segment.b - segment.a;
        let rotated = Vec2::new(-a_to_b.y, a_to_b.x);
        let normal = rotated.normalized() * self.inner.width;

        let mut clone = segment;
        clone.a += normal;
        clone.b += normal;
        self.real_api.segment(clone);

        segment.a -= normal;
        segment.b -= normal;
        self.real_api.segment(segment);

        self.real_api.segment((segment.a, clone.a).into());
        self.real_api.segment((segment.b, clone.b).into());
    }

    fn arc(&mut self, mut arc: Arc) {
        let mut cm = self.inner.cm.get();
        if cm.is_live() {
            cm.arc(&arc);
            self.inner.cm.set(cm);
        }

        arc.radius += self.inner.width;
        let [big_start, big_stop] = arc.ends();
        self.real_api.arc(arc);

        arc.radius -= self.inner.width;
        arc.radius -= self.inner.width;
        let [lil_start, lil_stop] = arc.ends();
        self.real_api.arc(arc);

        self.real_api.segment((big_start, lil_start).into());
        self.real_api.segment((big_stop, lil_stop).into());
    }
}

impl Shape for InitialStriker<'_> {
    fn draw(&self, api: &mut dyn PathApi) {
        let mut wrapper = StrikerApi {
            inner: self,
            real_api: api,
        };

        self.shape.draw(&mut wrapper);
    }
}

/////////////////////////////////////////////////////////

struct StrikerApi2<'a, C: Canvas + ?Sized> {
    canvas: &'a mut C,
    texture: &'a Texture<'a>,
    ssaa: SsaaConfig,
    width: f32,
}

impl<C: Canvas + ?Sized> StrikerApi2<'_, C> {
    fn point(&mut self, point: Point) {
        let disk = Disk {
            center: point,
            radius: self.width,
        };

        self.canvas.draw(&disk, self.texture, self.ssaa);
    }
}

impl<C: Canvas + ?Sized> PathApi for StrikerApi2<'_, C> {
    fn segment(&mut self, segment: Segment) {
        self.point(segment.a);
        self.point(segment.b);
    }

    fn arc(&mut self, arc: Arc) {
        let [a, b] = arc.ends();
        self.point(a);
        self.point(b);
    }
}

/////////////////////////////////////////////////////////

pub fn stroke<C: Canvas + ?Sized>(canvas: &mut C, shape: &dyn Shape, width: f32, texture: &Texture, ssaa: SsaaConfig) {
    let mut monitor = Cell::default();

    let striker = InitialStriker {
        shape,
        width,

        cm: &monitor,
    };

    canvas.draw(&striker, texture, ssaa);

    let cm = monitor.get_mut();

    // if curvature isn't continuous
    if !cm.is_live() || !cm.finalize() {
        let mut striker2 = StrikerApi2 {
            canvas,
            texture,
            ssaa,
            width,
        };

        shape.draw(&mut striker2);
    }
}