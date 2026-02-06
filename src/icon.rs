use iced::window::icon::{self, Icon};

/// Generate the application window icon (32x32 RGBA).
///
/// Draws two connected port nodes (green + amber) with a
/// cyan solder point, matching the app's color palette.
pub fn app_icon() -> Option<Icon> {
    const S: u32 = 32;
    let mut rgba = vec![0u8; (S * S * 4) as usize];

    for y in 0..S {
        for x in 0..S {
            let idx = ((y * S + x) * 4) as usize;
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            // Background
            let (mut r, mut g, mut b): (f32, f32, f32) = (0.082, 0.082, 0.102);

            // Left node (green) at (9, 16), radius 4.5
            let d = dist(fx, fy, 9.0, 16.0);
            if d < 5.0 {
                let t = smoothstep(5.0, 4.0, d);
                // Ring
                let ring = smoothstep(5.0, 4.0, d) - smoothstep(3.5, 2.5, d);
                blend(&mut r, &mut g, &mut b, 0.35, 0.75, 0.45, ring * 0.9);
                // Fill
                let fill = smoothstep(2.8, 1.8, d);
                blend(&mut r, &mut g, &mut b, 0.35, 0.75, 0.45, fill);
                // Dark center ring
                let dark = smoothstep(4.2, 3.5, d) - smoothstep(3.0, 2.2, d);
                blend(&mut r, &mut g, &mut b, 0.11, 0.11, 0.13, dark * 0.8);
                // Glow
                let _ = t;
            }
            // Glow around left node
            let glow = smoothstep(9.0, 5.0, d) * 0.15;
            blend(&mut r, &mut g, &mut b, 0.35, 0.75, 0.45, glow);

            // Right node (amber) at (23, 16), radius 4.5
            let d = dist(fx, fy, 23.0, 16.0);
            if d < 5.0 {
                let ring = smoothstep(5.0, 4.0, d) - smoothstep(3.5, 2.5, d);
                blend(&mut r, &mut g, &mut b, 0.92, 0.66, 0.25, ring * 0.9);
                let fill = smoothstep(2.8, 1.8, d);
                blend(&mut r, &mut g, &mut b, 0.92, 0.66, 0.25, fill);
                let dark = smoothstep(4.2, 3.5, d) - smoothstep(3.0, 2.2, d);
                blend(&mut r, &mut g, &mut b, 0.11, 0.11, 0.13, dark * 0.8);
            }
            let glow = smoothstep(9.0, 5.0, d) * 0.15;
            blend(&mut r, &mut g, &mut b, 0.92, 0.66, 0.25, glow);

            // Connection trace: bezier from (9,16) through (16,10) to (23,16)
            let trace_d = bezier_dist(fx, fy, 9.0, 16.0, 16.0, 10.0, 23.0, 16.0);
            let trace = smoothstep(2.0, 1.0, trace_d);
            // Gradient color along x: green -> cyan -> amber
            let t = ((fx - 9.0) / 14.0).clamp(0.0, 1.0);
            let (tr, tg, tb) = if t < 0.5 {
                let s = t * 2.0;
                (
                    lerp(0.35, 0.30, s),
                    lerp(0.75, 0.75, s),
                    lerp(0.45, 0.85, s),
                )
            } else {
                let s = (t - 0.5) * 2.0;
                (
                    lerp(0.30, 0.92, s),
                    lerp(0.75, 0.66, s),
                    lerp(0.85, 0.25, s),
                )
            };
            blend(&mut r, &mut g, &mut b, tr, tg, tb, trace * 0.85);

            // Center solder point (cyan) at (16, 10)
            let d = dist(fx, fy, 16.0, 10.0);
            if d < 3.5 {
                let ring = smoothstep(3.5, 2.8, d) - smoothstep(2.2, 1.5, d);
                blend(&mut r, &mut g, &mut b, 0.30, 0.75, 0.85, ring * 0.8);
                let fill = smoothstep(1.8, 0.8, d);
                blend(&mut r, &mut g, &mut b, 0.30, 0.75, 0.85, fill);
                let bright = smoothstep(1.0, 0.0, d);
                blend(&mut r, &mut g, &mut b, 1.0, 1.0, 1.0, bright * 0.6);
            }
            let glow = smoothstep(6.0, 3.5, d) * 0.12;
            blend(&mut r, &mut g, &mut b, 0.30, 0.75, 0.85, glow);

            rgba[idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
            rgba[idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
            rgba[idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
            rgba[idx + 3] = 255;
        }
    }

    icon::from_rgba(rgba, S, S).ok()
}

fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn blend(r: &mut f32, g: &mut f32, b: &mut f32, sr: f32, sg: f32, sb: f32, a: f32) {
    *r = lerp(*r, sr, a);
    *g = lerp(*g, sg, a);
    *b = lerp(*b, sb, a);
}

/// Approximate minimum distance from point (px,py) to a quadratic bezier
/// defined by control points (x0,y0), (x1,y1), (x2,y2).
fn bezier_dist(
    px: f32, py: f32,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
    x2: f32, y2: f32,
) -> f32 {
    let steps = 16;
    let mut min_d = f32::MAX;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let inv = 1.0 - t;
        let bx = inv * inv * x0 + 2.0 * inv * t * x1 + t * t * x2;
        let by = inv * inv * y0 + 2.0 * inv * t * y1 + t * t * y2;
        let d = dist(px, py, bx, by);
        if d < min_d {
            min_d = d;
        }
    }
    min_d
}
