use super::{Point, Color};

/// Image handle for use in a [`Canvas`]
#[derive(Copy, Clone, Debug)]
pub struct BitmapHandle(pub(crate) usize);

impl BitmapHandle {
    /// Forge a [`BitmapHandle`]
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn forge(inner: usize) -> Self {
        Self(inner)
    }

    /// Leak the inner usize
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn leak(&self) -> usize {
        self.0
    }
}

/// Mask handle for use in a [`Canvas`]
#[derive(Copy, Clone, Debug)]
pub struct MaskHandle(pub(crate) usize);

impl MaskHandle {
    /// Forge a [`MaskHandle`]
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn forge(inner: usize) -> Self {
        Self(inner)
    }

    /// Leak the inner usize
    ///
    /// This function is made available for implementations of [`Canvas`] outside of this crate.
    pub unsafe fn leak(&self) -> usize {
        self.0
    }
}

/// The expected content of a filled path
#[derive(Copy, Clone, Debug)]
pub enum Texture<'a> {
    SolidColor(Color),
    /// Not supported yet
    Gradient(&'a [(Point, Color)]),
    Mask {
        top_left: Point,
        scale: f32,
        repeat: bool,
        mask: MaskHandle,
        zero: Option<Color>,
        full: Color,
    },
    Bitmap {
        top_left: Point,
        scale: f32,
        repeat: bool,
        bitmap: BitmapHandle,
    },
    QuadBitmap {
        top_left: Point,
        btm_left: Point,
        top_right: Point,
        btm_right: Point,
        bitmap: BitmapHandle,
    },
    Debug,
}
