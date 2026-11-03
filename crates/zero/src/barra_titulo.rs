//! The window's own title bar (3 Nov 2026).
//!
//! The owner, 3 Nov 2026, inside the VPN: «el margen de arriba es demasiado ancho. Hazlo un poco más fino para que la
//! pantalla se vea más centrada… Y arriba del todo me sigue saliendo un candado a la izquierda y el botón de cerrar,
//! maximizar… mira a ver cómo pones eso de forma profesional… que quede bien de diseño y a la misma vez ganemos más
//! espacio en pantalla». The window had two bands on top: Windows' own title bar (the padlock, minimise, maximise,
//! close) and, under it, the top row of the page. Now there is one, the way first-class desktop programs do it: the top
//! row of what is on screen (ZERO's bar, or GUARDIANA's web and panel, which fill the window) is the title bar. Windows
//! draws no title bar of its own (`WM_NCCALCSIZE`) and keeps its resizing borders on the left, right and bottom; the
//! program draws the three buttons itself, at the top right, over the page, [`ALTO`] CSS pixels high; the empty parts of
//! that top row move the window (`app-region: drag`, which the engine turns into Windows' own move: drag, double click,
//! the system menu, snapping to an edge).
//!
//! Pure arithmetic and drawing here, tested on every system: the size of the band and of the buttons at each scale,
//! which part of them is under the pointer, the client area without Windows' title bar, and the pixels of the three
//! buttons in each state. The window (`zero/navegador`) puts them on screen.

use crate::ventana::Rect;

/// The height of the title bar, in CSS pixels: the buttons' height, the top row of ZERO's bar, and the top row of
/// GUARDIANA's web and panel inside the program.
pub const ALTO: i32 = 44;
/// The width of each of the window's buttons, in CSS pixels (Windows 11's own).
pub const ANCHO_BOTON: i32 = 46;
/// The width the three buttons take at the right of the title bar, in CSS pixels: the pages keep it free.
pub const ANCHO_BOTONES: i32 = 3 * ANCHO_BOTON;
/// The drawings' size, in CSS pixels (Windows 11 draws its own at 10).
const LADO_DIBUJO: i32 = 10;

/// One of the window's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boton {
    /// Minimise.
    Minimizar,
    /// Maximise, or restore while maximised.
    Maximizar,
    /// Close.
    Cerrar,
}

impl Boton {
    /// The three, from left to right.
    pub const TODOS: [Self; 3] = [Self::Minimizar, Self::Maximizar, Self::Cerrar];

    const fn indice(self) -> i32 {
        match self {
            Self::Minimizar => 0,
            Self::Maximizar => 1,
            Self::Cerrar => 2,
        }
    }
}

/// What is under the pointer on the program's own parts of the title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zona {
    /// A button.
    Boton(Boton),
    /// The top edge: it resizes the window upwards.
    Arriba,
    /// The top left corner: it resizes both ways.
    ArribaIzquierda,
    /// The top right corner: it resizes both ways.
    ArribaDerecha,
    /// Nothing of the title bar.
    Nada,
}

/// Device pixels for CSS pixels at a scale (device pixels per CSS pixel, the monitor's DPI over 96).
fn px(css: i32, escala: f64) -> i32 {
    (f64::from(css) * escala).round() as i32
}

/// The width of a line one CSS pixel thick, in whole device pixels (never less than one): the drawings' strokes, and the
/// line under the page's top row, which stays visible under the buttons.
#[must_use]
pub fn grueso(escala: f64) -> i32 {
    px(1, escala).max(1)
}

/// The buttons' strip at a scale: its width and height, in device pixels.
#[must_use]
pub fn tamano(escala: f64) -> (i32, i32) {
    (px(ANCHO_BOTONES, escala), px(ALTO, escala))
}

/// The left and right edges of a button inside the strip, in device pixels. Edges are rounded, never widths, so the
/// three meet exactly at any scale (a rounded width leaves a pixel's gap at 125 %).
#[must_use]
pub fn bordes(b: Boton, escala: f64) -> (i32, i32) {
    let i = b.indice();
    (
        px(i * ANCHO_BOTON, escala),
        px((i + 1) * ANCHO_BOTON, escala),
    )
}

/// What is under the pointer at (`x`, `y`) of the strip, in device pixels from its top left corner. `borde` is the
/// height of the window's top resizing edge, 0 while maximised: there, as on any window of Windows, the top edge resizes
/// the window, also over the buttons, and its last pixels at the right are the corner.
#[must_use]
pub fn zona(x: i32, y: i32, escala: f64, borde: i32) -> Zona {
    let (ancho, alto) = tamano(escala);
    if x < 0 || y < 0 || x >= ancho || y >= alto {
        return Zona::Nada;
    }
    if y < borde {
        return if x >= ancho - 2 * borde {
            Zona::ArribaDerecha
        } else {
            Zona::Arriba
        };
    }
    Boton::TODOS
        .into_iter()
        .find(|b| {
            let (l, r) = bordes(*b, escala);
            x >= l && x < r
        })
        .map_or(Zona::Nada, Zona::Boton)
}

/// What is under the pointer at `x` of the window's top edge, left of the buttons (the thin strip that resizes the
/// window while it is not maximised, `borde` pixels high): its first pixels at the left are the corner.
#[must_use]
pub const fn zona_borde(x: i32, borde: i32) -> Zona {
    if x < 2 * borde {
        Zona::ArribaIzquierda
    } else {
        Zona::Arriba
    }
}

/// The client area without Windows' title bar (`WM_NCCALCSIZE`): what Windows gives with its standard frame
/// (`estandar`: its borders off the left, right and bottom, and its title bar off the top), with the top back at the
/// window's own top (`arriba`, the top of the rectangle Windows proposed). A maximised window hangs `borde` pixels over
/// every edge of its screen (see [`crate::ventana::maximizada`]); those come off the top too, or the top row would
/// start above the screen.
#[must_use]
pub fn cliente(estandar: Rect, arriba: i32, maximizada: bool, borde: i32) -> Rect {
    Rect {
        top: if maximizada {
            arriba + borde.max(0)
        } else {
            arriba
        },
        ..estandar
    }
}

/// How the buttons are now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Estado {
    /// The button under the pointer.
    pub encima: Option<Boton>,
    /// The button held down (it is lit only while the pointer is still on it).
    pub pulsado: Option<Boton>,
    /// The window is maximised: the middle button restores, and says so with its drawing.
    pub maximizada: bool,
    /// The window is the one in front: its drawings dim when it is not, as Windows' own.
    pub activa: bool,
}

/// The page's ink for the drawings; brighter under the pointer; dimmer while another window is in front.
const TINTA: [u8; 3] = [0xC9, 0xD3, 0xDD];
const TINTA_ENCIMA: [u8; 3] = [0xF4, 0xF6, 0xF8];
const TINTA_INACTIVA: [u8; 3] = [0x6F, 0x7B, 0x88];
/// Behind a button under the pointer, white at this alpha (out of 255), as Windows 11 over a dark title bar; held down,
/// fainter.
const BLANCO_ENCIMA: u8 = 0x17;
const BLANCO_PULSADO: u8 = 0x0E;
/// Close: Windows' own red under the pointer, a darker red held down, and the cross in white.
const ROJO: [u8; 3] = [0xC4, 0x2B, 0x1C];
const ROJO_PULSADO: [u8; 3] = [0xA8, 0x26, 0x19];
const BLANCO: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// Where nothing is lit: black at alpha 1. Invisible over any page, but a layered window lets the pointer through the
/// pixels it leaves fully transparent, and the buttons must take it.
pub const CASI_TRANSPARENTE: u32 = 0x0100_0000;

/// One premultiplied BGRA pixel as Windows' layered windows read it (`0xAARRGGBB`), from premultiplied components.
fn empaqueta([r, g, b, a]: [f64; 4]) -> u32 {
    let c = |x: f64| u32::from(x.round().clamp(0.0, 255.0) as u8);
    (c(a) << 24) | (c(r) << 16) | (c(g) << 8) | c(b)
}

/// The premultiplied components of a pixel: red, green, blue, alpha.
fn desempaqueta(p: u32) -> [f64; 4] {
    let c = |k: u32| f64::from((p >> k) & 0xFF);
    [c(16), c(8), c(0), c(24)]
}

/// An opaque colour as premultiplied components.
fn opaco([r, g, b]: [u8; 3]) -> [f64; 4] {
    [f64::from(r), f64::from(g), f64::from(b), 255.0]
}

/// `arriba` over `abajo`, both premultiplied, with `k` of `arriba` (0 to 1).
fn sobre(arriba: [f64; 4], k: f64, abajo: [f64; 4]) -> [f64; 4] {
    let mut out = [0.0; 4];
    for (i, o) in out.iter_mut().enumerate() {
        *o = arriba[i] * k + abajo[i] * (1.0 - k * arriba[3] / 255.0);
    }
    out
}

/// A piece of a drawing, in device pixels (fractions allowed): a filled box, or a line with square ends.
#[derive(Debug, Clone, Copy)]
enum Trazo {
    Caja([f64; 4]),
    Linea([f64; 2], [f64; 2], f64),
}

impl Trazo {
    /// A box of whole pixels: left, top, width, height.
    fn caja(x: i32, y: i32, ancho: i32, alto: i32) -> Self {
        Self::Caja([
            f64::from(x),
            f64::from(y),
            f64::from(x + ancho),
            f64::from(y + alto),
        ])
    }

    fn cubre(&self, x: f64, y: f64) -> bool {
        match *self {
            Self::Caja([x0, y0, x1, y1]) => x >= x0 && x < x1 && y >= y0 && y < y1,
            Self::Linea([ax, ay], [bx, by], medio) => {
                let (dx, dy) = (bx - ax, by - ay);
                let largo2 = dx * dx + dy * dy;
                if largo2 <= 0.0 {
                    return false;
                }
                let t = ((x - ax) * dx + (y - ay) * dy) / largo2;
                if !(0.0..=1.0).contains(&t) {
                    return false;
                }
                ((x - ax) * dy - (y - ay) * dx).abs() / largo2.sqrt() <= medio
            }
        }
    }
}

/// The drawing of a button whose box is `l`..`r` across and `0`..`alto` down (device pixels), at a scale.
fn dibujo(b: Boton, maximizada: bool, l: i32, r: i32, alto: i32, escala: f64) -> Vec<Trazo> {
    let g = px(LADO_DIBUJO, escala).max(4);
    let w = grueso(escala);
    let x0 = l + (r - l - g) / 2;
    let y0 = (alto - w - g) / 2;
    match b {
        Boton::Minimizar => vec![Trazo::caja(x0, y0 + (g - w) / 2, g, w)],
        Boton::Maximizar if !maximizada => vec![
            Trazo::caja(x0, y0, g, w),
            Trazo::caja(x0, y0 + g - w, g, w),
            Trazo::caja(x0, y0, w, g),
            Trazo::caja(x0 + g - w, y0, w, g),
        ],
        Boton::Maximizar => {
            // Restore: a square in front at the bottom left, and the edges of the one behind it that show.
            let d = px(2, escala).max(2);
            let lado = g - d;
            let (fx, fy) = (x0, y0 + d);
            vec![
                Trazo::caja(fx, fy, lado, w),
                Trazo::caja(fx, fy + lado - w, lado, w),
                Trazo::caja(fx, fy, w, lado),
                Trazo::caja(fx + lado - w, fy, w, lado),
                Trazo::caja(x0 + d, y0, lado, w),
                Trazo::caja(x0 + g - w, y0, w, lado),
                Trazo::caja(x0 + d, y0, w, d),
                Trazo::caja(x0 + lado, y0 + lado - w, d, w),
            ]
        }
        Boton::Cerrar => {
            let (a, z) = (f64::from(x0), f64::from(x0 + g));
            let (arriba, abajo) = (f64::from(y0), f64::from(y0 + g));
            let medio = f64::from(w) / 2.0;
            vec![
                Trazo::Linea([a, arriba], [z, abajo], medio),
                Trazo::Linea([z, arriba], [a, abajo], medio),
            ]
        }
    }
}

/// How much of the pixel at (`x`, `y`) a drawing covers, from 0 to 1: four by four points inside it.
fn cobertura(trazos: &[Trazo], x: i32, y: i32) -> f64 {
    const N: i32 = 4;
    let mut dentro = 0u32;
    for j in 0..N {
        for i in 0..N {
            let px = f64::from(x) + (f64::from(i) + 0.5) / f64::from(N);
            let py = f64::from(y) + (f64::from(j) + 0.5) / f64::from(N);
            if trazos.iter().any(|t| t.cubre(px, py)) {
                dentro += 1;
            }
        }
    }
    f64::from(dentro) / f64::from(N * N)
}

/// The three buttons, drawn at a scale: [`tamano`] pixels, row by row from the top, each a premultiplied BGRA pixel as
/// Windows' layered windows take it (`0xAARRGGBB`). Where nothing is lit the strip is [`CASI_TRANSPARENTE`], so the page
/// under it shows; the last row of each button (one CSS pixel) is never lit, so the line under the page's top row runs
/// on under the buttons.
#[must_use]
pub fn dibuja(escala: f64, e: Estado) -> Vec<u32> {
    let (ancho, alto) = tamano(escala);
    let filas = usize::try_from(alto).unwrap_or(0);
    let columnas = usize::try_from(ancho).unwrap_or(0);
    let mut pixeles = vec![CASI_TRANSPARENTE; filas * columnas];
    let w = grueso(escala);
    let indice = |x: i32, y: i32| -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < columnas && y < filas).then_some(y * columnas + x)
    };
    for b in Boton::TODOS {
        let (l, r) = bordes(b, escala);
        let encima = e.encima == Some(b) && (e.pulsado.is_none() || e.pulsado == Some(b));
        let pulsado = e.pulsado == Some(b) && e.encima == Some(b);
        let fondo = match (b, pulsado, encima) {
            (Boton::Cerrar, true, _) => Some(opaco(ROJO_PULSADO)),
            (Boton::Cerrar, false, true) => Some(opaco(ROJO)),
            (_, true, _) => Some([f64::from(BLANCO_PULSADO); 4]),
            (_, false, true) => Some([f64::from(BLANCO_ENCIMA); 4]),
            _ => None,
        };
        if let Some(f) = fondo {
            let lleno = empaqueta(sobre(f, 1.0, desempaqueta(CASI_TRANSPARENTE)));
            for y in 0..alto - w {
                for x in l..r {
                    if let Some(i) = indice(x, y) {
                        pixeles[i] = lleno;
                    }
                }
            }
        }
        let tinta = if b == Boton::Cerrar && (encima || pulsado) {
            BLANCO
        } else if encima || pulsado {
            TINTA_ENCIMA
        } else if e.activa {
            TINTA
        } else {
            TINTA_INACTIVA
        };
        let trazos = dibujo(b, e.maximizada, l, r, alto, escala);
        for y in 0..alto - w {
            for x in l..r {
                let k = cobertura(&trazos, x, y);
                if k <= 0.0 {
                    continue;
                }
                if let Some(i) = indice(x, y) {
                    pixeles[i] = empaqueta(sobre(opaco(tinta), k, desempaqueta(pixeles[i])));
                }
            }
        }
    }
    pixeles
}

/// The thin strip along the rest of the top edge, which only takes the pointer: [`CASI_TRANSPARENTE`] all over.
#[must_use]
pub fn dibuja_borde(ancho: i32, alto: i32) -> Vec<u32> {
    let n = usize::try_from(ancho.max(0)).unwrap_or(0) * usize::try_from(alto.max(0)).unwrap_or(0);
    vec![CASI_TRANSPARENTE; n]
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESCALAS: [f64; 9] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0, 3.5];

    fn pixel(p: &[u32], escala: f64, x: i32, y: i32) -> u32 {
        let (ancho, _) = tamano(escala);
        let i = usize::try_from(y * ancho + x).unwrap_or(usize::MAX);
        p.get(i).copied().unwrap_or(0)
    }

    fn rgb(p: u32) -> (u32, u32, u32) {
        ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF)
    }

    #[test]
    fn the_band_and_the_buttons_at_each_scale() {
        assert_eq!(tamano(1.0), (138, 44));
        assert_eq!(tamano(1.25), (173, 55));
        assert_eq!(tamano(1.5), (207, 66));
        assert_eq!(tamano(2.0), (276, 88));
        assert_eq!(ANCHO_BOTONES, 138);
        for s in ESCALAS {
            let (ancho, _) = tamano(s);
            let (a, b, c) = (
                bordes(Boton::Minimizar, s),
                bordes(Boton::Maximizar, s),
                bordes(Boton::Cerrar, s),
            );
            // The three meet with no gap and end at the strip's edge, at every scale.
            assert_eq!((a.0, a.1, b.1, c.1), (0, b.0, c.0, ancho), "{s}");
            assert!(a.1 - a.0 >= 45 && c.1 - c.0 >= 45, "{s}");
        }
        assert_eq!(grueso(1.0), 1);
        assert_eq!(grueso(1.25), 1);
        assert_eq!(grueso(1.5), 2);
        assert_eq!(grueso(2.0), 2);
    }

    #[test]
    fn what_the_pointer_is_over() {
        // Restored: the top edge resizes, over the buttons too, and its last pixels at the right are the corner.
        assert_eq!(zona(10, 2, 1.0, 8), Zona::Arriba);
        assert_eq!(zona(130, 2, 1.0, 8), Zona::ArribaDerecha);
        assert_eq!(zona(10, 20, 1.0, 8), Zona::Boton(Boton::Minimizar));
        assert_eq!(zona(46, 20, 1.0, 8), Zona::Boton(Boton::Maximizar));
        assert_eq!(zona(137, 43, 1.0, 8), Zona::Boton(Boton::Cerrar));
        // Maximised: the buttons reach the screen's top, where the pointer stops (Fitts).
        assert_eq!(zona(130, 0, 1.0, 0), Zona::Boton(Boton::Cerrar));
        assert_eq!(zona(0, 0, 1.0, 0), Zona::Boton(Boton::Minimizar));
        // Outside the strip.
        assert_eq!(zona(138, 20, 1.0, 8), Zona::Nada);
        assert_eq!(zona(-1, 20, 1.0, 8), Zona::Nada);
        assert_eq!(zona(20, 44, 1.0, 8), Zona::Nada);
        // At 150 %: everything one and a half times as large.
        assert_eq!(zona(206, 65, 1.5, 0), Zona::Boton(Boton::Cerrar));
        assert_eq!(zona(69, 30, 1.5, 12), Zona::Boton(Boton::Maximizar));
        // The rest of the top edge: the left corner, then the edge.
        assert_eq!(zona_borde(3, 8), Zona::ArribaIzquierda);
        assert_eq!(zona_borde(16, 8), Zona::Arriba);
    }

    #[test]
    fn the_client_area_starts_at_the_windows_top() {
        let estandar = Rect::new(8, 39, 1192, 792);
        // Restored: the top back where the window starts; the other three edges keep Windows' borders.
        assert_eq!(cliente(estandar, 0, false, 8), Rect::new(8, 0, 1192, 792));
        // Maximised, the window hangs 8 pixels over the screen's top: the top row starts on screen.
        assert_eq!(cliente(estandar, -8, true, 8), Rect::new(8, 0, 1192, 792));
        assert_eq!(cliente(estandar, -11, true, 11), Rect::new(8, 0, 1192, 792));
        assert_eq!(cliente(estandar, 0, true, -3), Rect::new(8, 0, 1192, 792));
    }

    #[test]
    fn at_rest_the_strip_shows_only_the_drawings_and_still_takes_the_pointer() {
        for s in ESCALAS {
            let (ancho, alto) = tamano(s);
            let p = dibuja(
                s,
                Estado {
                    activa: true,
                    ..Estado::default()
                },
            );
            assert_eq!(p.len(), usize::try_from(ancho * alto).unwrap_or(0), "{s}");
            // Every pixel takes the pointer (alpha at least 1) and most of them are only that.
            assert!(p.iter().all(|x| x >> 24 >= 1), "{s}");
            let vacios = p.iter().filter(|x| **x == CASI_TRANSPARENTE).count();
            assert!(vacios * 10 > p.len() * 9, "{s}: {vacios} of {}", p.len());
            // The corners of each button are empty.
            for b in Boton::TODOS {
                let (l, r) = bordes(b, s);
                assert_eq!(pixel(&p, s, l + 1, 1), CASI_TRANSPARENTE, "{s} {b:?}");
                assert_eq!(
                    pixel(&p, s, r - 2, alto - 2),
                    CASI_TRANSPARENTE,
                    "{s} {b:?}"
                );
            }
        }
    }

    #[test]
    fn each_drawing_is_where_windows_puts_it() {
        let s = 1.0;
        let p = dibuja(
            s,
            Estado {
                activa: true,
                ..Estado::default()
            },
        );
        let opaco_de = |x: i32, y: i32| pixel(&p, s, x, y) >> 24 == 0xFF;
        // Minimise: one line across the middle of its button, 10 pixels long and 1 thick.
        let (l, r) = bordes(Boton::Minimizar, s);
        let fila: Vec<i32> = (0..44).filter(|y| opaco_de((l + r) / 2, *y)).collect();
        assert_eq!(fila.len(), 1, "{fila:?}");
        let y = fila[0];
        assert!((19..=23).contains(&y), "{y}");
        assert_eq!((l..r).filter(|x| opaco_de(*x, y)).count(), 10);
        assert_eq!(rgb(pixel(&p, s, (l + r) / 2, y)), (0xC9, 0xD3, 0xDD));
        // Maximise: a square outline of 10 by 10, empty inside.
        let (l, r) = bordes(Boton::Maximizar, s);
        let tinta: Vec<(i32, i32)> = (0..44)
            .flat_map(|y| (l..r).map(move |x| (x, y)))
            .filter(|(x, y)| opaco_de(*x, *y))
            .collect();
        assert_eq!(tinta.len(), 36, "{tinta:?}");
        let xs = tinta.iter().map(|(x, _)| *x);
        let ys = tinta.iter().map(|(_, y)| *y);
        let (x0, x1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
        let (y0, y1) = (ys.clone().min().unwrap_or(0), ys.max().unwrap_or(0));
        assert_eq!((x1 - x0, y1 - y0), (9, 9));
        assert!(!opaco_de(x0 + 5, y0 + 5));
        // Centred in its button, and level with minimise.
        assert!(((x0 + x1) - (l + r - 1)).abs() <= 1, "{x0} {x1} {l} {r}");
        assert!((((y0 + y1) / 2) - y).abs() <= 1);
        // Close: a cross of 10 by 10, its two lines meeting in the middle.
        let (l, r) = bordes(Boton::Cerrar, s);
        let cx = l + (r - l - 10) / 2;
        assert!(pixel(&p, s, cx + 5, y0 + 5) >> 24 > 0x80);
        assert!(pixel(&p, s, cx, y0) >> 24 > 0x60);
        assert!(pixel(&p, s, cx + 9, y0) >> 24 > 0x60);
        assert!(pixel(&p, s, cx + 9, y0 + 9) >> 24 > 0x60);
        assert_eq!(pixel(&p, s, cx + 5, y0), CASI_TRANSPARENTE);
    }

    #[test]
    fn under_the_pointer_and_held_down() {
        let s = 1.0;
        let (_, alto) = tamano(s);
        let reposo = dibuja(
            s,
            Estado {
                activa: true,
                ..Estado::default()
            },
        );
        // Close under the pointer: Windows' red behind a white cross, all over its button but the last row.
        let p = dibuja(
            s,
            Estado {
                encima: Some(Boton::Cerrar),
                activa: true,
                ..Estado::default()
            },
        );
        let (l, r) = bordes(Boton::Cerrar, s);
        assert_eq!(pixel(&p, s, l + 2, 2), 0xFFC4_2B1C);
        assert_eq!(pixel(&p, s, r - 1, alto - 2), 0xFFC4_2B1C);
        assert_eq!(pixel(&p, s, l + 2, alto - 1), CASI_TRANSPARENTE);
        let centro = pixel(&p, s, (l + r) / 2, 20);
        assert!(rgb(centro).1 > 0xA0, "{centro:08X}");
        // The other two are as at rest.
        let (_, fin) = bordes(Boton::Maximizar, s);
        for y in 0..alto {
            for x in 0..fin {
                assert_eq!(pixel(&p, s, x, y), pixel(&reposo, s, x, y));
            }
        }
        // Held down: darker.
        let p = dibuja(
            s,
            Estado {
                encima: Some(Boton::Cerrar),
                pulsado: Some(Boton::Cerrar),
                activa: true,
                ..Estado::default()
            },
        );
        assert_eq!(pixel(&p, s, l + 2, 2), 0xFFA8_2619);
        // Minimise under the pointer: a faint white behind it, the page still showing through.
        let p = dibuja(
            s,
            Estado {
                encima: Some(Boton::Minimizar),
                activa: true,
                ..Estado::default()
            },
        );
        let f = pixel(&p, s, 2, 2);
        assert!((0x10..0x30).contains(&(f >> 24)), "{f:08X}");
        // Pressed on one and moved onto another: neither is lit as pressed, the one pressed stays as it was.
        let p = dibuja(
            s,
            Estado {
                encima: Some(Boton::Maximizar),
                pulsado: Some(Boton::Cerrar),
                activa: true,
                ..Estado::default()
            },
        );
        assert_eq!(pixel(&p, s, l + 2, 2), CASI_TRANSPARENTE);
        let (ml, _) = bordes(Boton::Maximizar, s);
        assert_eq!(pixel(&p, s, ml + 2, 2), CASI_TRANSPARENTE);
    }

    #[test]
    fn maximised_the_middle_one_restores_and_another_window_in_front_dims_them() {
        let s = 1.0;
        let normal = dibuja(
            s,
            Estado {
                activa: true,
                ..Estado::default()
            },
        );
        let maxi = dibuja(
            s,
            Estado {
                activa: true,
                maximizada: true,
                ..Estado::default()
            },
        );
        let (l, r) = bordes(Boton::Maximizar, s);
        let distintos = (0..44)
            .flat_map(|y| (l..r).map(move |x| (x, y)))
            .filter(|(x, y)| pixel(&normal, s, *x, *y) != pixel(&maxi, s, *x, *y))
            .count();
        assert!(distintos > 8, "{distintos}");
        // The other two do not change.
        let (cl, _) = bordes(Boton::Cerrar, s);
        assert!((0..44).all(|y| (0..l)
            .chain(cl..138)
            .all(|x| pixel(&normal, s, x, y) == pixel(&maxi, s, x, y))));
        // Another window in front: the same drawings, dimmer.
        let apagada = dibuja(s, Estado::default());
        let suma = |p: &[u32]| -> u64 {
            p.iter()
                .map(|x| {
                    let (r, g, b) = rgb(*x);
                    u64::from(r + g + b)
                })
                .sum()
        };
        assert!(suma(&apagada) < suma(&normal));
        assert!(apagada.iter().all(|x| x >> 24 >= 1));
    }

    #[test]
    fn the_edge_strip_only_takes_the_pointer() {
        let p = dibuja_borde(1200, 8);
        assert_eq!(p.len(), 9600);
        assert!(p.iter().all(|x| *x == CASI_TRANSPARENTE));
        assert!(dibuja_borde(-5, 8).is_empty());
    }
}
