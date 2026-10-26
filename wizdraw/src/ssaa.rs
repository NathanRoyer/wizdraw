/// Super-Sampling Anti-Aliasing Configuration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SsaaConfig {
    None,
    X2,
    X4,
    X8,
    X16,
}

pub trait ConstSsaa {
    const LOG2_ROWS: u8;
    const LOG2_COLS: u8;
    const ROWS: u8;
    const COLS: u8;
    const SUBP: u8;
    const MASK: u32;
    const Y_BIAS: f32;
    const X_BIAS: f32;
    const Y_STEP: f32;
    const X_STEP: f32;
}

macro_rules! const_ssaa {
    ($struct:ident, $l2c:expr, $l2r:expr, $xb:expr, $yb:expr, $xs:expr, $ys:expr) => {
        pub struct $struct;

        impl ConstSsaa for $struct {
            const LOG2_ROWS: u8 = $l2r;
            const LOG2_COLS: u8 = $l2c;
            const ROWS: u8 = 1 << $l2r;
            const COLS: u8 = 1 << $l2c;
            const SUBP: u8 = Self::ROWS * Self::COLS;
            const MASK: u32 = (1 << Self::SUBP) - 1;
            const Y_BIAS: f32 = $xb;
            const X_BIAS: f32 = $yb;
            const Y_STEP: f32 = $xs;
            const X_STEP: f32 = $ys;
        }
    }
}

const_ssaa!(NoSsaa,  0, 0,  0.000,  0.000, 1.00, 1.00);
const_ssaa!(SsaaX2,  1, 0, -0.250,  0.000, 0.50, 1.00);
const_ssaa!(SsaaX4,  1, 1, -0.250, -0.250, 0.50, 0.50);
const_ssaa!(SsaaX8,  2, 1, -0.375, -0.250, 0.25, 0.50);
const_ssaa!(SsaaX16, 2, 2, -0.375, -0.375, 0.25, 0.25);
