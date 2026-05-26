type MaskRow = [u32; 8];

#[derive(Copy, Clone, Debug)]
pub struct Mask {
    rows: [MaskRow; 128],
}

#[inline(always)]
fn dword_bitmask(x: u8) -> (usize, u32) {
    let dword = x / 32;
    let shift = x % 32;
    (dword as usize, 1 << shift)
}

impl Mask {
    pub fn new() -> Self {
        Self {
            rows: [[0u32; 8]; 128],
        }
    }

    pub fn clear(&mut self) {
        for row in self.rows.iter_mut() {
            row.fill(0);
        }
    }

    pub fn toggle(&mut self, x: u8, y: u8) {
        let (dword, bitmask) = dword_bitmask(x);
        self.rows[y as usize][dword] ^= bitmask;
    }

    pub fn get(&self, x: u8, y: u8) -> bool {
        let (dword, bitmask) = dword_bitmask(x);
        (self.rows[y as usize][dword] & bitmask) != 0
    }

    #[allow(unused)]
    pub fn set(&mut self, x: u8, y: u8, value: bool) {
        let (dword, bitmask) = dword_bitmask(x);
        let ptr = &mut self.rows[y as usize][dword];
        match value {
            true => *ptr |= bitmask,
            false => *ptr &= !bitmask,
        }
    }

    pub fn unpack(&mut self) -> bool {
        let mut dirty = false;

        for row in self.rows.iter_mut() {
            let mut drawing = false;

            for dword in row.iter_mut().rev() {
                if *dword == 0 {
                    if drawing {
                        *dword = !0u32;
                    }

                    continue;
                }

                let mut bitmask = 1 << 31;
                let mut out = 0;

                for _ in 0..32 {
                    let is_set = (*dword & bitmask) > 0;
                    drawing ^= is_set;
                    dirty |= drawing;
                    out = out << 1;
                    out |= drawing as u32;
                    bitmask = bitmask >> 1;
                }

                *dword = out;
            }
        }

        dirty
    }
}
