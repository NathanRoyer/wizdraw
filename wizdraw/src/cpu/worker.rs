use super::*;

const CAP: usize = 64;

const INIT_POINT: Point = Point::new(-65536.0, 0.0);

type MultiSampler = fn([Point; CAP], &[CubicBezier], bool) -> [bool; CAP];

pub fn seq_sample(points: [Point; CAP], path: &[CubicBezier], holes: bool) -> [bool; CAP] {
    points.map(|p| seq::subpixel_is_in_path(p, path, holes))
}

#[cfg(feature = "simd")]
pub fn simd_sample(points: [Point; CAP], path: &[CubicBezier], holes: bool) -> [bool; CAP] {
    const L: usize = 4;
    let mut i = 0;
    let mut out = [false; CAP];

    while i < CAP {
        let points: [Point; L] = points[i..][..L].try_into().unwrap();
        let results = simd::subpixel_opacity::<L>(points, path, holes);
        out[i..][..L].copy_from_slice(&results);
        i += L;
    }

    out
}

#[derive(Copy, Clone, Debug)]
pub struct Worker {
    index: [usize; CAP],
    point: [Point; CAP],
    next: usize,
    callback: MultiSampler,
}

impl Worker {
    pub fn new(callback: MultiSampler) -> Self {
        Self {
            index: [0; CAP],
            point: [INIT_POINT; CAP],
            next: 0,
            callback,
        }
    }

    pub fn queue_ssaa(&mut self, mask_index: usize, point: Point, ssaa: SsaaConfig) {
        match ssaa {
            SsaaConfig::None => self.queue_all::< 1>(mask_index, point),
            SsaaConfig::X2   => self.queue_all::< 2>(mask_index, point),
            SsaaConfig::X4   => self.queue_all::< 4>(mask_index, point),
            SsaaConfig::X8   => self.queue_all::< 8>(mask_index, point),
            SsaaConfig::X16  => self.queue_all::<16>(mask_index, point),
        }
    }

    fn queue_all<const P: usize>(&mut self, mask_index: usize, pixel: Point) {
        for offset in ssaa_subpixel_map::<P>() {
            self.queue_one(mask_index, pixel + Point::from(*offset));
        }
    }

    fn queue_one(&mut self, mask_index: usize, point: Point) {
        self.point[self.next] = point;
        self.index[self.next] = mask_index;
        self.next += 1;
    }

    pub fn try_advance(
        &mut self,
        path: &[CubicBezier],
        mask: &mut [u8],
        holes: bool,
    ) {
        if self.next == CAP {
            self.force_advance(path, mask, holes);
        }
    }

    pub fn force_advance(
        &mut self,
        path: &[CubicBezier],
        mask: &mut [u8],
        holes: bool,
    ) {
        self.point[self.next..].fill(INIT_POINT);
        let results = (self.callback)(self.point, path, holes);

        for i in 0..self.next {
            let j = self.index[i];
            mask[j] += results[i] as u8;
        }

        self.next = 0;
    }
}
