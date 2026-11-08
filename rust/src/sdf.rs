//! Converts anti-aliased glyph coverage into a signed distance field (Felzenszwalb EDT).

const INF: f32 = 1e20;

/// 1D squared-distance transform of `f` into `d` (Felzenszwalb & Huttenlocher).
fn edt_1d(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    let mut k: usize = 0;
    v[0] = 0;
    z[0] = -INF;
    z[1] = INF;
    for q in 1..n {
        let fq = f[q] + (q * q) as f32;
        let mut s = (fq - (f[v[k]] + (v[k] * v[k]) as f32)) / (2.0 * (q - v[k]) as f32);
        // z[0] is -INF and |s| < INF, so k never underflows.
        while k > 0 && s <= z[k] {
            k -= 1;
            s = (fq - (f[v[k]] + (v[k] * v[k]) as f32)) / (2.0 * (q - v[k]) as f32);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = INF;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate().take(n) {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f32 - p as f32;
        *out = dq * dq + f[p];
    }
}

/// 2D squared Euclidean distance to the nearest "seed" pixel (grid value 0).
fn edt_2d(grid: &mut [f32], w: usize, h: usize) {
    let n = w.max(h);
    let mut f = vec![0f32; n];
    let mut d = vec![0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0f32; n + 1];
    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        edt_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..y * w + w]);
        edt_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        grid[y * w..y * w + w].copy_from_slice(&d[..w]);
    }
}

/// Returns an 8-bit SDF where 128 is the glyph edge and `spread` texels map to the full range.
pub fn coverage_to_sdf(cov: &[u8], w: usize, h: usize, spread: f32) -> Vec<u8> {
    let len = w * h;
    let mut outside = vec![0f32; len]; // distance from outside pixels to ink
    let mut inside = vec![0f32; len]; // distance from ink pixels to background
    for i in 0..len {
        let a = cov[i] as f32 / 255.0;
        outside[i] = if a >= 0.5 { 0.0 } else { INF };
        inside[i] = if a < 0.5 { 0.0 } else { INF };
    }
    edt_2d(&mut outside, w, h);
    edt_2d(&mut inside, w, h);
    let mut out = vec![0u8; len];
    for i in 0..len {
        let a = cov[i] as f32 / 255.0;
        let d = if a > 0.02 && a < 0.98 {
            0.5 - a // sub-texel estimate on the anti-aliased rim
        } else if a >= 0.5 {
            -(inside[i].sqrt() - 0.5)
        } else {
            outside[i].sqrt() - 0.5
        };
        let s = 0.5 - d / (2.0 * spread);
        out[i] = (s.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    out
}
