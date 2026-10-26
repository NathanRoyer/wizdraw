#[derive(Copy, Clone, Debug)]
pub struct Mask {
    rows: [u32; 8 * 128],
}

// one row = 8 u32s = 256 bits

fn dword_bitmask(x: u8, y: u8) -> (usize, u8) {
    let dword = (x / 32) as u16 + (y as u16 * 8);
    let shift = x % 32;
    (dword as usize, shift)
}

impl Mask {
    pub fn new() -> Self {
        Self {
            rows: [0u32; 8 * 128],
        }
    }

    pub fn clear(&mut self) {
        self.rows.fill(0);
    }

    pub fn toggle(&mut self, x: u8, y: u8) {
        let (dword, shift) = dword_bitmask(x, y);
        self.rows[dword] ^= 1 << shift;
    }

    pub fn get(&self, x: u8, y: u8, num: u8) -> u32 {
        let bitmask = (1 << num) - 1;
        let (dword, shift) = dword_bitmask(x, y);
        (self.rows[dword] >> shift) & bitmask
    }

    pub fn empty_rows(&self, y: u8, num: u8) -> bool {
        let start = y as usize * 8;
        let stop = start + (num as usize * 8);
        self.rows[start..stop].iter().all(|dword| *dword == 0)
    }

    pub fn unpack(&mut self) {
        for row in self.rows.chunks_mut(8).rev() {
            let mut drawing = false;

            for dword in row.iter_mut().rev() {
                if *dword == 0 {
                    if drawing {
                        *dword = 0xffffffff;
                    }

                    continue;
                }

                let mut bitmask = 1 << 31;
                let mut out = 0;

                for _ in 0..32 {
                    let is_set = (*dword & bitmask) > 0;
                    drawing ^= is_set;
                    out = out << 1;
                    out |= drawing as u32;
                    bitmask = bitmask >> 1;
                }

                *dword = out;
            }
        }
    }
}
