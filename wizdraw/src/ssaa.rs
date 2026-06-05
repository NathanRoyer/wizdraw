

/// these must be sorted in ascending order
pub type HorizontalOffsets = &'static [f32];
pub type SubPixelOffsets = &'static [(f32, HorizontalOffsets)];

const HO_4P: HorizontalOffsets = &[-0.375, -0.125, 0.125, 0.375];
const HO_2P: HorizontalOffsets = &[-0.25, 0.25];

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

    pub(crate) fn log2_rows<T: From<u8>>(&self) -> T {
        match self {
            SsaaConfig::None => 0,
            SsaaConfig::X2   => 1,
            SsaaConfig::X4   => 1,
            SsaaConfig::X8   => 2,
            SsaaConfig::X16  => 2,
        }.into()
    }

    pub(crate) fn log2_cols<T: From<u8>>(&self) -> T {
        match self {
            SsaaConfig::None => 0,
            SsaaConfig::X2   => 0,
            SsaaConfig::X4   => 1,
            SsaaConfig::X8   => 1,
            SsaaConfig::X16  => 2,
        }.into()
    }

    pub(crate) fn rows<T: From<u8>>(&self) -> T {
        T::from(1u8 << self.log2_rows::<u8>())
    }

    pub(crate) fn cols<T: From<u8>>(&self) -> T {
        T::from(1u8 << self.log2_cols::<u8>())
    }

    pub(crate) const fn offsets(&self) -> SubPixelOffsets {
        match self {
            Self::None => &[(0.0, &[0.0])],
            Self::X2 => &[
                (-0.25, &[0.0]),
                ( 0.25, &[0.0]),
            ],
            Self::X4 => &[
                (-0.25, HO_2P),
                ( 0.25, HO_2P),
            ],
            Self::X8 => &[
                (-0.375, HO_2P),
                (-0.125, HO_2P),
                ( 0.125, HO_2P),
                ( 0.375, HO_2P),
            ],
            Self::X16 => &[
                (-0.375, HO_4P),
                (-0.125, HO_4P),
                ( 0.125, HO_4P),
                ( 0.375, HO_4P),
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
