//! Where the window goes when it is maximised (1.0.46).
//!
//! The owner, 18 Oct 2026, on a 34" monitor: «abajo se ve cortado». ZERO maximised reached under
//! the Windows taskbar, so the bottom of the globe's tray was hidden behind it. A maximised
//! window must fill the monitor's *work area* (the monitor minus the taskbar, wherever it is),
//! on the monitor the window is on, at that monitor's scale. When the taskbar hides itself, the
//! work area is the whole monitor, and a window that covers every pixel of the taskbar's edge
//! keeps it from ever coming back: one pixel row is left free on that edge, as Windows' own
//! windows do. Pure arithmetic here, tested on every system; the window asks it in
//! `WM_GETMINMAXINFO`.

/// A rectangle in screen pixels: left, top, right, bottom (right and bottom excluded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    #[must_use]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    #[must_use]
    pub const fn ancho(&self) -> i32 {
        self.right - self.left
    }
    #[must_use]
    pub const fn alto(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Edges of the monitor where a taskbar that hides itself lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Ocultas {
    /// On the left edge.
    pub izquierda: bool,
    /// On the top edge.
    pub arriba: bool,
    /// On the right edge.
    pub derecha: bool,
    /// On the bottom edge.
    pub abajo: bool,
}

/// The maximised window, as `MINMAXINFO` wants it: the position relative to the monitor's
/// top-left corner and the size. `borde` is the width of the resizing frame at the monitor's
/// scale: a maximised window with a standard frame hangs that much outside the area on every
/// side, so its client area is exactly the work area.
#[must_use]
pub fn maximizada(
    monitor: Rect,
    trabajo: Rect,
    borde: i32,
    ocultas: Ocultas,
) -> ((i32, i32), (i32, i32)) {
    let mut t = trabajo;
    // Nonsense from the system (an empty or outside work area): the whole monitor.
    if t.ancho() <= 0
        || t.alto() <= 0
        || t.left < monitor.left
        || t.top < monitor.top
        || t.right > monitor.right
        || t.bottom > monitor.bottom
    {
        t = monitor;
    }
    // A taskbar that hides itself only on the edges the work area does not already leave.
    if t == monitor {
        if ocultas.izquierda {
            t.left += 1;
        }
        if ocultas.arriba {
            t.top += 1;
        }
        if ocultas.derecha {
            t.right -= 1;
        }
        if ocultas.abajo {
            t.bottom -= 1;
        }
    }
    let b = borde.max(0);
    (
        (t.left - monitor.left - b, t.top - monitor.top - b),
        (t.ancho() + 2 * b, t.alto() + 2 * b),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: Rect = Rect::new(0, 0, 3440, 1440);

    #[test]
    fn taskbar_at_the_bottom_is_left_free() {
        let (pos, tam) = maximizada(M, Rect::new(0, 0, 3440, 1392), 8, Ocultas::default());
        assert_eq!(pos, (-8, -8));
        assert_eq!(tam, (3456, 1408));
        // The client area (size minus the frame on both sides) ends where the taskbar begins.
        assert_eq!(pos.1 + tam.1 - 8, 1392);
    }

    #[test]
    fn taskbar_on_any_edge() {
        let (pos, tam) = maximizada(M, Rect::new(62, 0, 3440, 1440), 8, Ocultas::default());
        assert_eq!((pos.0 + 8, tam.0 - 16), (62, 3378));
        let (pos, tam) = maximizada(M, Rect::new(0, 48, 3440, 1440), 8, Ocultas::default());
        assert_eq!((pos.1 + 8, tam.1 - 16), (48, 1392));
        let (pos, tam) = maximizada(M, Rect::new(0, 0, 3378, 1440), 0, Ocultas::default());
        assert_eq!((pos, tam), ((0, 0), (3378, 1440)));
    }

    #[test]
    fn a_second_monitor_is_measured_from_its_own_corner() {
        // A monitor left of the main one, at another scale, taskbar at its bottom.
        let m = Rect::new(-2560, 200, 0, 1640);
        let (pos, tam) = maximizada(m, Rect::new(-2560, 200, 0, 1580), 11, Ocultas::default());
        assert_eq!(pos, (-11, -11));
        assert_eq!(tam, (2560 + 22, 1380 + 22));
    }

    #[test]
    fn a_hidden_taskbar_keeps_one_pixel_to_come_back() {
        let o = Ocultas {
            abajo: true,
            ..Ocultas::default()
        };
        let (pos, tam) = maximizada(M, M, 8, o);
        assert_eq!(pos, (-8, -8));
        assert_eq!(tam, (3456, 1440 - 1 + 16));
        // With a visible taskbar somewhere else, nothing more is taken.
        let (_, tam) = maximizada(M, Rect::new(0, 0, 3440, 1392), 8, o);
        assert_eq!(tam.1, 1392 + 16);
    }

    #[test]
    fn nonsense_from_the_system_falls_back_to_the_monitor() {
        let (pos, tam) = maximizada(M, Rect::default(), 8, Ocultas::default());
        assert_eq!((pos, tam), ((-8, -8), (3456, 1456)));
    }
}
