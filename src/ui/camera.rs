use macroquad::prelude::*;

pub struct Camera {
    /// World-space (tile units) point at screen center.
    pub center: (f32, f32),
    /// Pixels per tile.
    pub ppt: f32,
    drag_anchor: Option<(f32, f32)>,
    /// True if the most recent press turned into a drag. Also set when a
    /// press begins over UI, so that gesture never reads as a world click.
    /// Deliberately NOT cleared on release: release-frame readers
    /// (click-vs-drag detection in the inspector, which runs after
    /// Camera::update) rely on it. It resets on the next press. Do not read
    /// it outside a release-frame context.
    pub dragged: bool,
}

impl Camera {
    pub fn new(center: (f32, f32), ppt: f32) -> Camera {
        Camera { center, ppt, drag_anchor: None, dragged: false }
    }

    /// Camera showing all of a `world` (w, h in tiles) centered inside the
    /// screen-space rect `left..right` × `top..bottom`, zoomed to fit. Use the
    /// area the UI panels leave clear so none of them cover the world at spawn.
    pub fn fit(world: (f32, f32), left: f32, top: f32, right: f32, bottom: f32) -> Camera {
        const INSET: f32 = 12.0;
        let (cw, ch) = (right - left - INSET * 2.0, bottom - top - INSET * 2.0);
        let ppt = (cw / world.0).min(ch / world.1).clamp(6.0, 72.0);
        let mut cam = Camera::new((world.0 / 2.0, world.1 / 2.0), ppt);
        // `center` is the world point at the WINDOW center; shift it so the
        // world center lands on the rect's center instead.
        cam.center.0 += (screen_width() / 2.0 - (left + right) / 2.0) / ppt;
        cam.center.1 += (screen_height() / 2.0 - (top + bottom) / 2.0) / ppt;
        cam
    }

    pub fn to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        (
            (wx - self.center.0) * self.ppt + screen_width() / 2.0,
            (wy - self.center.1) * self.ppt + screen_height() / 2.0,
        )
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (
            (sx - screen_width() / 2.0) / self.ppt + self.center.0,
            (sy - screen_height() / 2.0) / self.ppt + self.center.1,
        )
    }

    /// Handle pan (left-drag) + zoom (wheel, toward cursor).
    /// `ui_hover`: pointer is over UI; ignore input then.
    pub fn update(&mut self, ui_hover: bool) {
        let (mx, my) = mouse_position();
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.0 && !ui_hover {
            let before = self.to_world(mx, my);
            self.ppt = (self.ppt * (1.0 + wheel.signum() * 0.12)).clamp(6.0, 72.0);
            let after = self.to_world(mx, my);
            self.center.0 += before.0 - after.0;
            self.center.1 += before.1 - after.1;
        }
        if is_mouse_button_pressed(MouseButton::Left) {
            if ui_hover {
                // Press began over UI: poison the gesture so release-frame
                // readers (inspector click detection) ignore it even if the
                // pointer leaves the UI before release.
                self.drag_anchor = None;
                self.dragged = true;
            } else {
                self.drag_anchor = Some((mx, my));
                self.dragged = false;
            }
        }
        if is_mouse_button_down(MouseButton::Left) {
            if let Some((ax, ay)) = self.drag_anchor {
                let (dx, dy) = (mx - ax, my - ay);
                if dx.abs() + dy.abs() > 4.0 {
                    self.dragged = true;
                }
                if self.dragged {
                    self.center.0 -= dx / self.ppt;
                    self.center.1 -= dy / self.ppt;
                    self.drag_anchor = Some((mx, my));
                }
            }
        } else {
            self.drag_anchor = None;
        }
    }
}
