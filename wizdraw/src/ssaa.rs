

pub type SubPixelOffsets = &'static [(f32, f32)];

/// Super-Sampling Anti-Aliasing Configuration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SsaaConfig {
    None,
    X2,
    X4,
    X8,
    X16,
}

impl SsaaConfig {
    pub(crate) fn as_mul<T: From<u8>>(&self) -> T {
        match self {
            SsaaConfig::None =>  1,
            SsaaConfig::X2   =>  2,
            SsaaConfig::X4   =>  4,
            SsaaConfig::X8   =>  8,
            SsaaConfig::X16  => 16,
        }.into()
    }

    pub(crate) const fn offsets(&self) -> SubPixelOffsets {
        match self {
            Self::None => &[(0.0, 0.0)],
            Self::X2 => &[(-0.25, -0.25), (0.25, 0.25)],
            Self::X4 => &[
                (-0.25, -0.25),
                (-0.25,  0.25),
                ( 0.25, -0.25),
                ( 0.25,  0.25),
            ],
            Self::X8 => &[
                (-0.125, -0.125),
                (-0.375, -0.375),

                (-0.125,  0.125),
                (-0.375,  0.375),

                ( 0.125, -0.125),
                ( 0.375, -0.375),

                ( 0.125,  0.125),
                ( 0.375,  0.375),
            ],
            Self::X16 => &[
                (-0.125, -0.125),
                (-0.375, -0.375),

                (-0.125,  0.125),
                (-0.375,  0.375),

                ( 0.125, -0.125),
                ( 0.375, -0.375),

                ( 0.125,  0.125),
                ( 0.375,  0.375),

                (-0.125, -0.375),
                (-0.375, -0.125),

                (-0.125,  0.375),
                (-0.375,  0.125),

                ( 0.125, -0.375),
                ( 0.375, -0.125),

                ( 0.125,  0.375),
                ( 0.375,  0.125),
            ],
        }
    }
}

// looks dumb but improves performance
// because the operand is a const
#[doc(hidden)]
#[macro_export]
macro_rules! const_ssaa {
    ($ssaa:expr, $a:expr, $op:tt) => {
        match $ssaa {
            SsaaConfig::None => $a $op 1,
            SsaaConfig::X2 => $a $op 2,
            SsaaConfig::X4 => $a $op 4,
            SsaaConfig::X8 => $a $op 8,
            SsaaConfig::X16 => $a $op 16,
        }
    }
}
