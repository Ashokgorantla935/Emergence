/// Cursor preview data computed each frame by the god tools system.
/// The renderer reads this to draw overlays before committing an action.
#[derive(Debug, Default, Clone)]
pub struct CursorPreview {
    /// World-space center of the brush/cursor
    pub world_pos: [f32; 2],
    /// Radius in world units (brush radius for area tools)
    pub radius: f32,
    /// Whether to show a filled circle (area brush) vs point cursor
    pub show_circle: bool,
    /// RGBA color for the preview overlay (0.0..1.0 each channel)
    pub color: [f32; 4],
    /// True if the cursor is over a valid placement position
    pub valid: bool,
    /// For drag tools: the drag start position
    pub drag_start: Option<[f32; 2]>,
    /// For drag tools: show a line from drag_start to world_pos
    pub show_drag_line: bool,
}

impl CursorPreview {
    pub fn new() -> Self {
        CursorPreview::default()
    }

    pub fn point(world_pos: [f32; 2], valid: bool) -> Self {
        CursorPreview {
            world_pos,
            radius: 0.5,
            show_circle: false,
            color: if valid { [0.2, 1.0, 0.2, 0.6] } else { [1.0, 0.2, 0.2, 0.6] },
            valid,
            drag_start: None,
            show_drag_line: false,
        }
    }

    pub fn brush(world_pos: [f32; 2], radius: f32, valid: bool, color: [f32; 4]) -> Self {
        CursorPreview {
            world_pos,
            radius,
            show_circle: true,
            color,
            valid,
            drag_start: None,
            show_drag_line: false,
        }
    }

    pub fn drag(world_pos: [f32; 2], start: [f32; 2]) -> Self {
        CursorPreview {
            world_pos,
            radius: 0.5,
            show_circle: false,
            color: [0.3, 0.7, 1.0, 0.7],
            valid: true,
            drag_start: Some(start),
            show_drag_line: true,
        }
    }
}
