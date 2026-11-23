//! A small vector rasterizer for the Tauri build's own pictures: its icons (24×24 outlines
//! from src/ui/components/Icons.tsx: strokes of 1.9 with round caps and joins, some filled)
//! and its logo (src/logo.svg). Each is drawn once at the size it is shown, in exact pixels
//! with smooth edges, and kept as a small texture; the SVG markup is read as the page has it.
//!
//! Only what those pictures use is read: <path> (M L H V C S A Z, absolute and relative),
//! <circle>, <ellipse>, <rect> with rounded corners; fill, stroke, stroke-width and fill-rule.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

fn p(x: f32, y: f32) -> P {
    P { x, y }
}

/// a shape flattened into polylines
#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub lines: Vec<(Vec<P>, bool)>,
    pub fill: bool,
    pub stroke: bool,
    pub stroke_width: f32,
    pub even_odd: bool,
}

// ---- path data ------------------------------------------------------------------------------

fn numbers(s: &str) -> Vec<f32> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut dot = c == b'.';
            while i < b.len() {
                let d = b[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == b'.' && !dot {
                    dot = true;
                    i += 1;
                } else if (d == b'e' || d == b'E') && i + 1 < b.len() {
                    i += 1;
                    if b[i] == b'-' || b[i] == b'+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(v) = s[start..i].parse::<f32>() {
                out.push(v);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn cubic(out: &mut Vec<P>, a: P, b: P, c: P, d: P) {
    // enough steps that the curve is smooth at the sizes icons are drawn
    let len = ((b.x - a.x).hypot(b.y - a.y) + (c.x - b.x).hypot(c.y - b.y) + (d.x - c.x).hypot(d.y - c.y)).max(1.0);
    let n = (len * 1.5).clamp(6.0, 64.0) as usize;
    for i in 1..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push(p(w0 * a.x + w1 * b.x + w2 * c.x + w3 * d.x, w0 * a.y + w1 * b.y + w2 * c.y + w3 * d.y));
    }
}

/// an SVG elliptical arc (the endpoint form) as points after `from`
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<P>, from: P, mut rx: f32, mut ry: f32, angle: f32, large: bool, sweep: bool, to: P) {
    if (from.x - to.x).abs() < 1e-6 && (from.y - to.y).abs() < 1e-6 {
        return;
    }
    if rx.abs() < 1e-6 || ry.abs() < 1e-6 {
        out.push(to);
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    let (s, c) = angle.to_radians().sin_cos();
    let dx = (from.x - to.x) / 2.0;
    let dy = (from.y - to.y) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (from.x + to.x) / 2.0;
    let cy = s * cx1 + c * cy1 + (from.y + to.y) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let a = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        let a = a.clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            -a
        } else {
            a
        }
    };
    let t1 = ang(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    let n = ((dt.abs() * rx.max(ry)).max(4.0) as usize).clamp(4, 96);
    for i in 1..=n {
        let t = t1 + dt * i as f32 / n as f32;
        let (st, ct) = t.sin_cos();
        out.push(p(c * rx * ct - s * ry * st + cx, s * rx * ct + c * ry * st + cy));
    }
}

/// path data as polylines (each with whether it was closed)
pub fn path(d: &str) -> Vec<(Vec<P>, bool)> {
    let mut lines: Vec<(Vec<P>, bool)> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let mut pos = p(0.0, 0.0);
    let mut start = pos;
    let mut last_ctrl: Option<P> = None;
    // split into commands with their numbers
    let mut cmds: Vec<(char, Vec<f32>)> = Vec::new();
    let mut at = 0;
    let chars: Vec<(usize, char)> = d.char_indices().collect();
    for (i, (bi, c)) in chars.iter().enumerate() {
        if c.is_ascii_alphabetic() && *c != 'e' && *c != 'E' {
            if i > 0 || at != *bi {
                if let Some(last) = cmds.last_mut() {
                    last.1 = numbers(&d[at..*bi]);
                }
            }
            cmds.push((*c, vec![]));
            at = bi + 1;
        }
    }
    if let Some(last) = cmds.last_mut() {
        last.1 = numbers(&d[at..]);
    }
    let flush = |cur: &mut Vec<P>, lines: &mut Vec<(Vec<P>, bool)>, closed: bool| {
        if cur.len() > 1 {
            lines.push((std::mem::take(cur), closed));
        } else {
            cur.clear();
        }
    };
    for (cmd, nums) in cmds {
        let rel = cmd.is_ascii_lowercase();
        let base = |pos: P| if rel { pos } else { p(0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                for (k, pair) in nums.chunks(2).enumerate() {
                    if pair.len() < 2 {
                        break;
                    }
                    let b = base(pos);
                    pos = p(b.x + pair[0], b.y + pair[1]);
                    if k == 0 {
                        flush(&mut cur, &mut lines, false);
                        start = pos;
                    }
                    cur.push(pos);
                }
                last_ctrl = None;
            }
            'L' => {
                for pair in nums.chunks(2).filter(|c| c.len() == 2) {
                    let b = base(pos);
                    pos = p(b.x + pair[0], b.y + pair[1]);
                    cur.push(pos);
                }
                last_ctrl = None;
            }
            'H' => {
                for v in nums {
                    pos = p(if rel { pos.x + v } else { v }, pos.y);
                    cur.push(pos);
                }
                last_ctrl = None;
            }
            'V' => {
                for v in nums {
                    pos = p(pos.x, if rel { pos.y + v } else { v });
                    cur.push(pos);
                }
                last_ctrl = None;
            }
            'C' => {
                for c in nums.chunks(6).filter(|c| c.len() == 6) {
                    let b = base(pos);
                    let (c1, c2, to) = (p(b.x + c[0], b.y + c[1]), p(b.x + c[2], b.y + c[3]), p(b.x + c[4], b.y + c[5]));
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    cubic(&mut cur, pos, c1, c2, to);
                    last_ctrl = Some(c2);
                    pos = to;
                }
            }
            'S' => {
                for c in nums.chunks(4).filter(|c| c.len() == 4) {
                    let b = base(pos);
                    let c1 = last_ctrl.map(|l| p(2.0 * pos.x - l.x, 2.0 * pos.y - l.y)).unwrap_or(pos);
                    let (c2, to) = (p(b.x + c[0], b.y + c[1]), p(b.x + c[2], b.y + c[3]));
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    cubic(&mut cur, pos, c1, c2, to);
                    last_ctrl = Some(c2);
                    pos = to;
                }
            }
            'A' => {
                for a in nums.chunks(7).filter(|c| c.len() == 7) {
                    let b = base(pos);
                    let to = p(b.x + a[5], b.y + a[6]);
                    if cur.is_empty() {
                        cur.push(pos);
                    }
                    arc(&mut cur, pos, a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0, to);
                    pos = to;
                }
                last_ctrl = None;
            }
            'Z' => {
                if !cur.is_empty() {
                    cur.push(start);
                }
                flush(&mut cur, &mut lines, true);
                pos = start;
                cur.push(pos);
                last_ctrl = None;
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut lines, false);
    lines
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<P> {
    let n = ((rx.max(ry) * 6.0) as usize).clamp(16, 96);
    (0..=n).map(|i| {
        let t = std::f32::consts::TAU * i as f32 / n as f32;
        p(cx + rx * t.cos(), cy + ry * t.sin())
    }).collect()
}

fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<P> {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return vec![p(x, y), p(x + w, y), p(x + w, y + h), p(x, y + h), p(x, y)];
    }
    let mut out = Vec::new();
    let corner = |out: &mut Vec<P>, cx: f32, cy: f32, a0: f32| {
        for i in 0..=8 {
            let t = (a0 + 90.0 * i as f32 / 8.0).to_radians();
            out.push(p(cx + r * t.cos(), cy + r * t.sin()));
        }
    };
    corner(&mut out, x + w - r, y + r, -90.0);
    corner(&mut out, x + w - r, y + h - r, 0.0);
    corner(&mut out, x + r, y + h - r, 90.0);
    corner(&mut out, x + r, y + r, 180.0);
    out.push(out[0]);
    out
}

// ---- markup ---------------------------------------------------------------------------------

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(&tag[i..j])
}

fn num(tag: &str, name: &str) -> f32 {
    attr(tag, name).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0)
}

/// the shapes of a piece of SVG markup; `stroke` and `width` are the outer <svg>'s defaults
/// (the icons: stroke="currentColor" strokeWidth=1.9, fill none)
pub fn shapes(markup: &str, stroke: bool, width: f32) -> Vec<Shape> {
    let mut out = Vec::new();
    for part in markup.split('<').skip(1) {
        let tag = format!(" {}", part.split('>').next().unwrap_or(""));
        let name = tag.split_whitespace().next().unwrap_or("");
        let lines = match name {
            "path" => path(attr(&tag, "d").unwrap_or("")),
            "circle" => {
                let r = num(&tag, "r");
                vec![(ellipse(num(&tag, "cx"), num(&tag, "cy"), r, r), true)]
            }
            "ellipse" => vec![(ellipse(num(&tag, "cx"), num(&tag, "cy"), num(&tag, "rx"), num(&tag, "ry")), true)],
            "rect" => vec![(round_rect(num(&tag, "x"), num(&tag, "y"), num(&tag, "width"), num(&tag, "height"), num(&tag, "rx")), true)],
            _ => continue,
        };
        let fill = attr(&tag, "fill").is_some_and(|f| f != "none");
        let stroked = match attr(&tag, "stroke") {
            Some("none") => false,
            Some(_) => true,
            None => stroke,
        };
        let w = attr(&tag, "stroke-width").or(attr(&tag, "strokeWidth")).and_then(|v| v.parse().ok()).unwrap_or(width);
        let even_odd = attr(&tag, "fill-rule").or(attr(&tag, "fillRule")) == Some("evenodd");
        out.push(Shape { lines, fill, stroke: stroked, stroke_width: w, even_odd });
    }
    out
}

// ---- drawing ---------------------------------------------------------------------------------

/// vertical samples per pixel row for fills
const SUB: usize = 4;

fn fill(shape: &Shape, s: f32, ox: f32, oy: f32, w: usize, h: usize, acc: &mut [f32]) {
    let mut edges: Vec<(P, P)> = Vec::new();
    for (line, _) in &shape.lines {
        // a fill always closes its outline
        for k in 0..line.len() {
            let a = line[k];
            let b = line[(k + 1) % line.len()];
            edges.push((p(a.x * s + ox, a.y * s + oy), p(b.x * s + ox, b.y * s + oy)));
        }
    }
    let mut cov = vec![0f32; w * h];
    let mut xs: Vec<(f32, i32)> = Vec::new();
    for row in 0..h {
        for k in 0..SUB {
            let y = row as f32 + (k as f32 + 0.5) / SUB as f32;
            xs.clear();
            for (a, b) in &edges {
                if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                    let t = (y - a.y) / (b.y - a.y);
                    xs.push((a.x + t * (b.x - a.x), if b.y > a.y { 1 } else { -1 }));
                }
            }
            xs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut wind = 0;
            for i in 0..xs.len() {
                let before = wind;
                wind += xs[i].1;
                let inside = |w: i32| if shape.even_odd { w % 2 != 0 } else { w != 0 };
                if !inside(before) && inside(wind) {
                    // a span starts here; find where it ends
                    let x0 = xs[i].0;
                    let mut wj = wind;
                    let mut x1 = x0;
                    for xj in xs.iter().skip(i + 1) {
                        wj += xj.1;
                        if !inside(wj) {
                            x1 = xj.0;
                            break;
                        }
                    }
                    let (x0, x1) = (x0.max(0.0), x1.min(w as f32));
                    if x1 <= x0 {
                        continue;
                    }
                    let (c0, c1) = (x0.floor() as usize, (x1.ceil() as usize).min(w));
                    for c in c0..c1 {
                        let a = (x1.min(c as f32 + 1.0) - x0.max(c as f32)).max(0.0);
                        cov[row * w + c] += a / SUB as f32;
                    }
                }
            }
        }
    }
    for (o, c) in acc.iter_mut().zip(cov) {
        *o = o.max(c.min(1.0));
    }
}

fn stroke(shape: &Shape, s: f32, ox: f32, oy: f32, w: usize, h: usize, acc: &mut [f32]) {
    let hw = shape.stroke_width * s / 2.0;
    for (line, _) in &shape.lines {
        let pts: Vec<P> = line.iter().map(|q| p(q.x * s + ox, q.y * s + oy)).collect();
        for k in 0..pts.len().saturating_sub(1).max(if pts.len() == 1 { 1 } else { 0 }) {
            let a = pts[k];
            let b = *pts.get(k + 1).unwrap_or(&a);
            let (minx, maxx) = (a.x.min(b.x) - hw - 1.0, a.x.max(b.x) + hw + 1.0);
            let (miny, maxy) = (a.y.min(b.y) - hw - 1.0, a.y.max(b.y) + hw + 1.0);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            for y in (miny.max(0.0) as usize)..(maxy.ceil().max(0.0) as usize).min(h) {
                for x in (minx.max(0.0) as usize)..(maxx.ceil().max(0.0) as usize).min(w) {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let t = if len2 > 0.0 { (((px - a.x) * dx + (py - a.y) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                    let d = (px - (a.x + t * dx)).hypot(py - (a.y + t * dy));
                    let c = (hw + 0.5 - d).clamp(0.0, 1.0);
                    let o = &mut acc[y * w + x];
                    if c > *o {
                        *o = c;
                    }
                }
            }
        }
    }
}

/// the shapes of a `view` (width, height) box drawn into `w`×`h` pixels, fitted and centred
/// (as an <img> shows an SVG); white with the coverage as alpha, premultiplied
pub fn rasterize(shapes: &[Shape], view: (f32, f32), w: usize, h: usize) -> Vec<u8> {
    let s = (w as f32 / view.0).min(h as f32 / view.1);
    let (ox, oy) = ((w as f32 - view.0 * s) / 2.0, (h as f32 - view.1 * s) / 2.0);
    let mut acc = vec![0f32; w * h];
    for sh in shapes {
        if sh.fill {
            fill(sh, s, ox, oy, w, h, &mut acc);
        }
        if sh.stroke {
            stroke(sh, s, ox, oy, w, h, &mut acc);
        }
    }
    acc.iter().flat_map(|a| {
        let v = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        [v, v, v, v]
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(px: &[u8], w: usize, x: usize, y: usize) -> u8 {
        px[(y * w + x) * 4 + 3]
    }

    #[test]
    fn reads_the_icons_path_data() {
        let l = path("M4 10.5L12 4l8 6.5V19a1.5 1.5 0 0 1-1.5 1.5h-4v-6h-5v6h-4A1.5 1.5 0 0 1 4 19z");
        assert_eq!(l.len(), 1);
        assert!(l[0].1);
        let first = l[0].0[0];
        assert_eq!((first.x, first.y), (4.0, 10.5));
        assert_eq!(numbers("1.5-1.5.5.5"), vec![1.5, -1.5, 0.5, 0.5]);
    }

    #[test]
    fn a_filled_triangle_and_a_stroke_land_where_they_should() {
        // the play icon at 24 px: solid inside, empty outside
        let sh = shapes(r#"<path d="M7 4.5v15l13-7.5z" fill="currentColor" stroke="none" />"#, true, 1.9);
        let px = rasterize(&sh, (24.0, 24.0), 24, 24);
        assert_eq!(alpha(&px, 24, 10, 12), 255);
        assert_eq!(alpha(&px, 24, 2, 2), 0);
        // a stroked line 1.9 wide: full in the middle, nothing far off
        let sh = shapes(r#"<path d="M6 12h12" />"#, true, 1.9);
        let px = rasterize(&sh, (24.0, 24.0), 24, 24);
        assert!(alpha(&px, 24, 12, 11) > 200 || alpha(&px, 24, 12, 12) > 200);
        assert_eq!(alpha(&px, 24, 12, 5), 0);
        // round caps reach a little past the ends
        assert!(alpha(&px, 24, 18, 11) > 0 || alpha(&px, 24, 18, 12) > 0);
    }

    #[test]
    fn even_odd_leaves_holes() {
        let sh = shapes(r#"<path fill-rule="evenodd" d="M0 0H10V10H0Z M3 3H7V7H3Z" fill="white"/>"#, false, 0.0);
        let px = rasterize(&sh, (10.0, 10.0), 10, 10);
        assert_eq!(alpha(&px, 10, 1, 1), 255);
        assert_eq!(alpha(&px, 10, 5, 5), 0);
    }
}
