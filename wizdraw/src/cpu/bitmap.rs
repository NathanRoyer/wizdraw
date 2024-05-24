use super::*;

#[derive(Debug, Clone)]
pub struct Bitmaps {
    library: Vec<Bitmap>,
    fallback: Bitmap,
}

impl Bitmaps {
    pub fn new() -> Self {
        let redish = Color::new(237, 47, 56, 255);
        let black = Color::new(0, 0, 0, 255);
        let mut pixels = Vec::with_capacity(100 * 100);

        for _ in 0..50 {
            pixels.extend_from_slice(&[black; 100]);
            pixels.extend_from_slice(&[redish; 100]);
        }

        Self {
            library: Vec::new(),
            fallback: Bitmap {
                size: Vec2::new(100, 100),
                pixels: pixels.into(),
            },
        }
    }

    pub fn push(&mut self, bitmap: Bitmap) -> BitmapHandle {
        let handle = BitmapHandle(self.library.len());
        self.library.push(bitmap);
        handle
    }

    pub fn get(&self, handle: BitmapHandle) -> &Bitmap {
        self.library.get(handle.0).unwrap_or(&self.fallback)
    }

    pub fn get_mut(&mut self, handle: BitmapHandle) -> Option<&mut Bitmap> {
        self.library.get_mut(handle.0)
    }
}