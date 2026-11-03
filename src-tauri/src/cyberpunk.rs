use serde_json::{json, Value};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};

const HEX_NAMES: [&str; 5] = ["1C", "E9", "55", "BD", "7A"];

// Embedded 48x40 matrix templates
const MAT_1C: &[u8] = include_bytes!("../templates/mat_1C.bin");
const MAT_E9: &[u8] = include_bytes!("../templates/mat_E9.bin");
const MAT_55: &[u8] = include_bytes!("../templates/mat_55.bin");
const MAT_BD: &[u8] = include_bytes!("../templates/mat_BD.bin");
const MAT_7A: &[u8] = include_bytes!("../templates/mat_7A.bin");

// Embedded 34x26 sequence templates
const SEQ_1C: &[u8] = include_bytes!("../templates/seq_1C.bin");
const SEQ_E9: &[u8] = include_bytes!("../templates/seq_E9.bin");
const SEQ_55: &[u8] = include_bytes!("../templates/seq_55.bin");
const SEQ_BD: &[u8] = include_bytes!("../templates/seq_BD.bin");
const SEQ_7A: &[u8] = include_bytes!("../templates/seq_7A.bin");

fn normalize(bytes: &[u8]) -> Vec<f32> {
    let len = bytes.len() as f32;
    let sum: f32 = bytes.iter().map(|&b| b as f32).sum();
    let mean = sum / len;
    let var: f32 = bytes.iter().map(|&b| {
        let diff = b as f32 - mean;
        diff * diff
    }).sum::<f32>() / len;
    let std = (var + 1e-5).sqrt();
    bytes.iter().map(|&b| (b as f32 - mean) / std).collect()
}

pub fn capture_screen(x: i32, y: i32, width: i32, height: i32) -> Option<Vec<u8>> {
    unsafe {
        let hdc_screen = GetDC(std::ptr::null_mut());
        if hdc_screen.is_null() {
            return None;
        }
        let hdc_mem = CreateCompatibleDC(hdc_screen);
        if hdc_mem.is_null() {
            ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return None;
        }
        let hbm = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbm.is_null() {
            DeleteDC(hdc_mem);
            ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return None;
        }
        let old_bm = SelectObject(hdc_mem, hbm);

        BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, x, y, SRCCOPY);

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width;
        bmi.bmiHeader.biHeight = -height; // top-down
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;

        let buf_size = (width * height * 4) as usize;
        let mut buffer = vec![0u8; buf_size];

        GetDIBits(
            hdc_mem,
            hbm,
            0,
            height as u32,
            buffer.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );

        SelectObject(hdc_mem, old_bm);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(std::ptr::null_mut(), hdc_screen);

        Some(buffer)
    }
}

fn get_gray(buf: &[u8], w: i32, h: i32, x: i32, y: i32) -> f32 {
    if x < 0 || x >= w || y < 0 || y >= h {
        return 0.0;
    }
    let idx = ((y * w + x) * 4) as usize;
    let b = buf[idx] as f32;
    let g = buf[idx + 1] as f32;
    let r = buf[idx + 2] as f32;
    r * 0.299 + g * 0.587 + b * 0.114
}

fn detect_header(buf: &[u8], w: i32, h: i32) -> Option<(i32, i32, i32, i32)> {
    let y_start = (h as f32 * 0.20) as i32;
    let y_end = (h as f32 * 0.50) as i32;
    let x_start = (w as f32 * 0.05) as i32;
    let x_end = (w as f32 * 0.55) as i32;

    let mut yellow_y_counts = vec![0i32; (y_end - y_start) as usize];
    for y in y_start..y_end {
        let mut count = 0;
        let yi = (y - y_start) as usize;
        for x in (x_start..x_end).step_by(2) {
            let idx = ((y * w + x) * 4) as usize;
            let b = buf[idx];
            let g = buf[idx + 1];
            let r = buf[idx + 2];
            if r > 180 && g > 200 && b < 100 {
                count += 1;
            }
        }
        yellow_y_counts[yi] = count;
    }

    let mut y_blocks: Vec<(i32, i32, i32)> = Vec::new();
    let mut in_block = false;
    let mut block_start = 0;
    for (i, &cnt) in yellow_y_counts.iter().enumerate() {
        let y = y_start + i as i32;
        if cnt > 150 && !in_block {
            in_block = true;
            block_start = y;
        } else if cnt <= 150 && in_block {
            in_block = false;
            y_blocks.push((block_start, y, y - block_start));
        }
    }
    if in_block {
        y_blocks.push((block_start, y_end, y_end - block_start));
    }

    if y_blocks.is_empty() {
        return None;
    }

    // Filter for the main matrix header block (typically 40-75px height; timer bar above is only ~10-15px)
    let header_block = y_blocks
        .iter()
        .filter(|b| b.2 >= 20 && b.2 <= 90)
        .max_by_key(|b| b.2)
        .or_else(|| y_blocks.iter().max_by_key(|b| b.2))?;

    let (hy1, hy2) = (header_block.0, header_block.1);
    if hy2 - hy1 < 15 {
        return None;
    }

    let mut x_counts = vec![0i32; w as usize];
    for y in hy1..hy2 {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let b = buf[idx];
            let g = buf[idx + 1];
            let r = buf[idx + 2];
            if r > 180 && g > 200 && b < 100 {
                x_counts[x as usize] += 1;
            }
        }
    }

    let threshold = ((hy2 - hy1) as f32 * 0.25) as i32;
    let mut in_block = false;
    let mut start_x = 0;
    let mut best_block = None;
    for x in 0..w {
        let cnt = x_counts[x as usize];
        if cnt >= threshold && !in_block {
            in_block = true;
            start_x = x;
        } else if cnt < threshold && in_block {
            in_block = false;
            let bw = x - start_x;
            if bw > 300 {
                best_block = Some((start_x, x));
                break;
            }
        }
    }
    if in_block && (w - start_x > 300) {
        best_block = Some((start_x, w));
    }

    let (hx1, hx2) = best_block?;
    Some((hx1, hy1, hx2 - hx1, hy2 - hy1))
}

fn match_patch(patch: &[f32], templates: &[Vec<f32>]) -> (&'static str, f32) {
    let len = patch.len() as f32;
    let sum: f32 = patch.iter().sum();
    let mean = sum / len;
    let var: f32 = patch.iter().map(|&x| {
        let diff = x - mean;
        diff * diff
    }).sum::<f32>() / len;
    let std = (var + 1e-5).sqrt();

    let mut best_name = HEX_NAMES[0];
    let mut best_corr = -2.0f32;

    for (name, t_norm) in HEX_NAMES.iter().zip(templates.iter()) {
        let mut corr_sum = 0.0f32;
        for i in 0..patch.len() {
            let p_norm = (patch[i] - mean) / std;
            corr_sum += p_norm * t_norm[i];
        }
        let corr = corr_sum / len;
        if corr > best_corr {
            best_corr = corr;
            best_name = name;
        }
    }
    (best_name, best_corr)
}

fn evaluate_path<'a>(codes: &[&'a str], sequences: &[Vec<&'a str>]) -> Vec<usize> {
    let mut matched = Vec::new();
    for (s_idx, seq) in sequences.iter().enumerate() {
        let mut ptr = 0;
        let mut completed = false;
        for &c in codes {
            if completed {
                break;
            }
            if c == seq[ptr] {
                ptr += 1;
                if ptr == seq.len() {
                    completed = true;
                }
            } else {
                ptr = if c == seq[0] { 1 } else { 0 };
            }
        }
        if completed {
            matched.push(s_idx);
        }
    }
    matched
}

fn score_matched(matched: &[usize]) -> i32 {
    let mut score = matched.len() as i32 * 1000;
    if matched.contains(&2) {
        score += 100;
    }
    if matched.contains(&1) {
        score += 30;
    }
    if matched.contains(&0) {
        score += 10;
    }
    score
}

fn solve_matrix<'a>(
    matrix: &'a [[&'a str; 6]; 6],
    sequences: &'a [Vec<&'a str>],
    buffer_size: usize,
) -> (Vec<(usize, usize)>, Vec<usize>) {
    let mut best_score = -1;
    let mut best_path = Vec::new();
    let mut best_matched = Vec::new();

    fn dfs<'a>(
        r: usize,
        c: usize,
        is_horiz: bool,
        path: &mut Vec<(usize, usize)>,
        visited: &mut [[bool; 6]; 6],
        matrix: &'a [[&'a str; 6]; 6],
        sequences: &'a [Vec<&'a str>],
        buffer_size: usize,
        best_score: &mut i32,
        best_path: &mut Vec<(usize, usize)>,
        best_matched: &mut Vec<usize>,
    ) {
        path.push((r, c));
        visited[r][c] = true;

        let codes: Vec<&str> = path.iter().map(|&(pr, pc)| matrix[pr][pc]).collect();
        let matched = evaluate_path(&codes, sequences);
        let score = score_matched(&matched);

        if score > *best_score || (score == *best_score && !best_path.is_empty() && path.len() < best_path.len()) {
            *best_score = score;
            *best_path = path.clone();
            *best_matched = matched.clone();
        }

        if matched.len() == sequences.len() || path.len() == buffer_size {
            path.pop();
            visited[r][c] = false;
            return;
        }

        if is_horiz {
            for nr in 0..6 {
                if nr != r && !visited[nr][c] {
                    dfs(nr, c, false, path, visited, matrix, sequences, buffer_size, best_score, best_path, best_matched);
                }
            }
        } else {
            for nc in 0..6 {
                if nc != c && !visited[r][nc] {
                    dfs(r, nc, true, path, visited, matrix, sequences, buffer_size, best_score, best_path, best_matched);
                }
            }
        }

        path.pop();
        visited[r][c] = false;
    }

    let mut path = Vec::with_capacity(buffer_size);
    let mut visited = [[false; 6]; 6];
    for start_c in 0..6 {
        dfs(
            0,
            start_c,
            true,
            &mut path,
            &mut visited,
            matrix,
            sequences,
            buffer_size,
            &mut best_score,
            &mut best_path,
            &mut best_matched,
        );
    }

    (best_path, best_matched)
}

fn scan_breach_internal(win_x: i32, win_y: i32, win_w: i32, win_h: i32, sf: f64) -> Option<Value> {
    let buf = capture_screen(win_x, win_y, win_w, win_h)?;
    let (hx1, hy1, _hw, _hh) = detect_header(&buf, win_w, win_h)?;

    let mat_tpls = vec![
        normalize(MAT_1C),
        normalize(MAT_E9),
        normalize(MAT_55),
        normalize(MAT_BD),
        normalize(MAT_7A),
    ];

    let seq_tpls = vec![
        normalize(SEQ_1C),
        normalize(SEQ_E9),
        normalize(SEQ_55),
        normalize(SEQ_BD),
        normalize(SEQ_7A),
    ];

    // Grid coordinates
    // Based on header position
    let row_ys = [
        hy1 + 113,
        hy1 + 199,
        hy1 + 284,
        hy1 + 370,
        hy1 + 455,
        hy1 + 541,
    ];
    let col_xs = [
        hx1 + 210,
        hx1 + 295,
        hx1 + 380,
        hx1 + 466,
        hx1 + 551,
        hx1 + 636,
    ];

    let mut matrix = [["1C"; 6]; 6];
    let mut matrix_coords = [[(0i32, 0i32); 6]; 6];

    for r in 0..6 {
        let cy = row_ys[r];
        for c in 0..6 {
            let cx = col_xs[c];
            // Extract 48x40 patch
            let mut patch = Vec::with_capacity(48 * 40);
            for py in (cy - 20)..(cy + 20) {
                for px in (cx - 24)..(cx + 24) {
                    patch.push(get_gray(&buf, win_w, win_h, px, py));
                }
            }
            let (best_name, _) = match_patch(&patch, &mat_tpls);
            matrix[r][c] = best_name;
            matrix_coords[r][c] = (cx, cy);
        }
    }

    // Sequence coordinates
    let seq_ys = [hy1 + 86, hy1 + 166, hy1 + 246];
    let seq_xs = [hx1 + 1014, hx1 + 1069, hx1 + 1125, hx1 + 1181];

    let mut sequences: Vec<Vec<&str>> = Vec::new();
    for &gy in &seq_ys {
        let mut seq = Vec::new();
        for &gx in &seq_xs {
            let mut patch = Vec::with_capacity(34 * 26);
            let mut sum_val = 0.0f32;
            let mut sum_sq = 0.0f32;
            for py in (gy - 13)..(gy + 13) {
                for px in (gx - 17)..(gx + 17) {
                    let val = get_gray(&buf, win_w, win_h, px, py);
                    patch.push(val);
                    sum_val += val;
                    sum_sq += val * val;
                }
            }
            let len = patch.len() as f32;
            let mean = sum_val / len;
            let std = (sum_sq / len - mean * mean + 1e-5).sqrt();
            if std < 15.0 {
                continue; // empty slot
            }
            let (best_name, corr) = match_patch(&patch, &seq_tpls);
            if corr > 0.40 {
                seq.push(best_name);
            }
        }
        if !seq.is_empty() {
            sequences.push(seq);
        }
    }

    if sequences.is_empty() {
        return None;
    }

    let buffer_size = 4;
    let (best_path, matched) = solve_matrix(&matrix, &sequences, buffer_size);

    let mut path_json = Vec::new();
    let mut path_codes = Vec::new();
    for (step_idx, &(r, c)) in best_path.iter().enumerate() {
        let (cx, cy) = matrix_coords[r][c];
        let code = matrix[r][c];
        path_codes.push(code);
        let render_x = if sf > 0.0 { (cx as f64 / sf).round() as i32 } else { cx };
        let render_y = if sf > 0.0 { (cy as f64 / sf).round() as i32 } else { cy };
        path_json.push(json!({
            "step": step_idx + 1,
            "row": r,
            "col": c,
            "x": render_x,
            "y": render_y,
            "screen_x": cx,
            "screen_y": cy,
            "code": code
        }));
    }

    let daemon_names = [
        "Обычная добыча данных (Eurodollars)",
        "Продвинутая добыча данных (Eurodollars + Components)",
        "Высокоуровневая добыча данных (Quickhack Blueprints)",
    ];
    let matched_names: Vec<&str> = matched.iter().map(|&idx| daemon_names.get(idx).unwrap_or(&"Daemon")).copied().collect();

    Some(json!({
        "active": true,
        "buffer_size": buffer_size,
        "matrix": matrix,
        "sequences": sequences,
        "path": path_json,
        "codes": path_codes,
        "daemons": matched_names,
        "matched_count": matched.len()
    }))
}

pub fn scan_and_solve_breach(win_x: i32, win_y: i32, win_w: i32, win_h: i32, sf: f64) -> Option<Value> {
    if let Some(res) = scan_breach_internal(win_x, win_y, win_w, win_h, sf) {
        return Some(res);
    }
    if win_x != 0 || win_y != 0 {
        if let Some(res) = scan_breach_internal(0, 0, 2560, 1440, sf) {
            return Some(res);
        }
    }
    None
}
