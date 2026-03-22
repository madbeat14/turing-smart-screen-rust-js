/// Frame diffing engine — only send changed regions to minimize serial traffic.
///
/// Divides the screen into tiles (default 32x32), compares current vs previous frame,
/// collects dirty tiles, and merges adjacent ones into larger rectangles.
/// Critical optimization: serial is only ~11.5 KB/s at 115200 baud.

/// A rectangular region of the display that needs updating
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirtyRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// Frame differ with configurable tile size
pub struct FrameDiffer {
    tile_size: u16,
    screen_width: u16,
    screen_height: u16,
    /// Previous frame's RGB565 data (2 bytes per pixel)
    prev_frame: Vec<u8>,
}

impl FrameDiffer {
    /// Create a new frame differ for the given screen dimensions
    pub fn new(screen_width: u16, screen_height: u16, tile_size: u16) -> Self {
        Self {
            tile_size,
            screen_width,
            screen_height,
            prev_frame: Vec::new(),
        }
    }

    /// Compare current frame against previous and return dirty rectangles.
    /// `current` is RGB565 data (2 bytes per pixel), same format as what goes to the display.
    /// Returns empty vec if frames are identical.
    pub fn diff(&mut self, current: &[u8]) -> Vec<DirtyRect> {
        let expected_size = (self.screen_width as usize) * (self.screen_height as usize) * 2;

        // First frame: mark everything dirty
        if self.prev_frame.len() != expected_size {
            self.prev_frame = current.to_vec();
            return vec![DirtyRect {
                x: 0,
                y: 0,
                w: self.screen_width,
                h: self.screen_height,
            }];
        }

        if current.len() != expected_size {
            self.prev_frame = current.to_vec();
            return vec![DirtyRect {
                x: 0,
                y: 0,
                w: self.screen_width,
                h: self.screen_height,
            }];
        }

        let tiles_x = (self.screen_width + self.tile_size - 1) / self.tile_size;
        let tiles_y = (self.screen_height + self.tile_size - 1) / self.tile_size;

        let mut dirty_tiles: Vec<bool> = vec![false; (tiles_x as usize) * (tiles_y as usize)];

        // Compare each tile
        let stride = (self.screen_width as usize) * 2; // bytes per row in RGB565
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                let tile_x = tx * self.tile_size;
                let tile_y = ty * self.tile_size;
                let tile_w = self.tile_size.min(self.screen_width - tile_x) as usize;
                let tile_h = self.tile_size.min(self.screen_height - tile_y) as usize;

                let mut is_dirty = false;
                'tile_check: for row in 0..tile_h {
                    let y_offset = (tile_y as usize + row) * stride;
                    let x_start = (tile_x as usize) * 2;
                    let start = y_offset + x_start;
                    let end = start + tile_w * 2;

                    if end <= current.len() && end <= self.prev_frame.len() {
                        if current[start..end] != self.prev_frame[start..end] {
                            is_dirty = true;
                            break 'tile_check;
                        }
                    }
                }

                if is_dirty {
                    dirty_tiles[(ty as usize) * (tiles_x as usize) + (tx as usize)] = true;
                }
            }
        }

        // Save current as previous for next diff
        self.prev_frame.clear();
        self.prev_frame.extend_from_slice(current);

        // Convert dirty tiles to rectangles, merging horizontally adjacent tiles
        let mut rects = Vec::new();
        for ty in 0..tiles_y as usize {
            let mut run_start: Option<usize> = None;

            for tx in 0..=tiles_x as usize {
                let is_dirty = if tx < tiles_x as usize {
                    dirty_tiles[ty * (tiles_x as usize) + tx]
                } else {
                    false
                };

                if is_dirty {
                    if run_start.is_none() {
                        run_start = Some(tx);
                    }
                } else if let Some(start) = run_start {
                    // End of a horizontal run — emit rectangle
                    let rect_x = (start as u16) * self.tile_size;
                    let rect_y = (ty as u16) * self.tile_size;
                    let rect_w = ((tx - start) as u16 * self.tile_size)
                        .min(self.screen_width - rect_x);
                    let rect_h = self.tile_size.min(self.screen_height - rect_y);

                    rects.push(DirtyRect {
                        x: rect_x,
                        y: rect_y,
                        w: rect_w,
                        h: rect_h,
                    });
                    run_start = None;
                }
            }
        }

        // Merge vertically adjacent rectangles with the same x range
        merge_vertical(&mut rects);

        rects
    }

    /// Check if the previous frame is empty (first frame)
    pub fn has_previous_frame(&self) -> bool {
        !self.prev_frame.is_empty()
    }

    /// Reset the differ (forces full refresh on next diff)
    pub fn reset(&mut self) {
        self.prev_frame.clear();
    }
}

/// Merge vertically adjacent rectangles that share the same x and width
fn merge_vertical(rects: &mut Vec<DirtyRect>) {
    if rects.len() < 2 {
        return;
    }

    // Sort by x, then y
    rects.sort_by(|a, b| a.x.cmp(&b.x).then(a.y.cmp(&b.y)));

    let mut merged = Vec::with_capacity(rects.len());
    let mut i = 0;
    while i < rects.len() {
        let mut current = rects[i].clone();
        while i + 1 < rects.len()
            && rects[i + 1].x == current.x
            && rects[i + 1].w == current.w
            && rects[i + 1].y == current.y + current.h
        {
            current.h += rects[i + 1].h;
            i += 1;
        }
        merged.push(current);
        i += 1;
    }

    *rects = merged;
}

/// Extract a rectangular sub-region from RGB565 frame data
pub fn extract_region(frame: &[u8], screen_width: u16, rect: &DirtyRect) -> Vec<u8> {
    let stride = (screen_width as usize) * 2;
    let region_stride = (rect.w as usize) * 2;
    let mut region = Vec::with_capacity(region_stride * rect.h as usize);

    for row in 0..rect.h as usize {
        let y_offset = (rect.y as usize + row) * stride;
        let x_offset = (rect.x as usize) * 2;
        let start = y_offset + x_offset;
        let end = start + region_stride;

        if end <= frame.len() {
            region.extend_from_slice(&frame[start..end]);
        }
    }

    region
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_frame_is_fully_dirty() {
        let mut differ = FrameDiffer::new(64, 64, 32);
        let frame = vec![0u8; 64 * 64 * 2];
        let rects = differ.diff(&frame);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], DirtyRect { x: 0, y: 0, w: 64, h: 64 });
    }

    #[test]
    fn test_identical_frames_no_dirty() {
        let mut differ = FrameDiffer::new(64, 64, 32);
        let frame = vec![0u8; 64 * 64 * 2];
        differ.diff(&frame); // first frame
        let rects = differ.diff(&frame); // same frame
        assert!(rects.is_empty());
    }

    #[test]
    fn test_single_tile_dirty() {
        let mut differ = FrameDiffer::new(64, 64, 32);
        let frame1 = vec![0u8; 64 * 64 * 2];
        differ.diff(&frame1);

        // Modify one pixel in the top-left tile
        let mut frame2 = frame1.clone();
        frame2[0] = 0xFF;
        let rects = differ.diff(&frame2);

        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[0].w, 32);
        assert_eq!(rects[0].h, 32);
    }

    #[test]
    fn test_extract_region() {
        // 4x4 screen, extract 2x2 region at (1,1)
        let mut frame = vec![0u8; 4 * 4 * 2];
        // Set pixel (2,2) to something non-zero
        frame[(2 * 4 + 2) * 2] = 0xAB;
        frame[(2 * 4 + 2) * 2 + 1] = 0xCD;

        let region = extract_region(
            &frame,
            4,
            &DirtyRect { x: 1, y: 1, w: 2, h: 2 },
        );
        assert_eq!(region.len(), 2 * 2 * 2); // 2x2 pixels * 2 bytes
        // Pixel (2,2) in screen = (1,1) in region
        assert_eq!(region[1 * 4 + 2], 0xAB);
        assert_eq!(region[1 * 4 + 3], 0xCD);
    }

    #[test]
    fn test_merge_vertical_rects() {
        let mut rects = vec![
            DirtyRect { x: 0, y: 0, w: 32, h: 32 },
            DirtyRect { x: 0, y: 32, w: 32, h: 32 },
        ];
        merge_vertical(&mut rects);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], DirtyRect { x: 0, y: 0, w: 32, h: 64 });
    }
}
