// WasmOS v2.0 — ブラウザで動く小さなデスクトップ OS（Rust + WebAssembly）
//
// v2.0 の主な変更:
// - ディスプレイを 1920x1080（Full HD）に高解像度化
// - フォントを全面作り直し（数字が全部同じ箱だった問題を修正、記号を大量追加）
// - ターミナル命令を大幅追加（help / ls / cat / touch / rm / mv / edit /
//   echo / calc / time / date / open / close / ps / mem / ver / brush など）
// - ターミナルの折り返しとスクロール（末尾表示）に対応
// - ファイルを localStorage に保存し、再読み込みしても残るように
// - タスクマネージャーが実状態（稼働時間・ウィンドウ数・ファイル数）を表示
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const VRAM_SIZE: usize = WIDTH * HEIGHT * 4;
const TASKBAR_H: i32 = 34;
const VERSION: &str = "2.0";

const PAINT_W: i32 = 360;
const PAINT_H: i32 = 360;
const PAINT_STRIDE: usize = 360;
const PALETTE: [u8; 8] = [0, 36, 73, 109, 146, 182, 219, 255];
const STORAGE_KEY: &str = "wasmos_files_v2";

struct WindowState {
    id: u8,
    title: String,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    is_open: bool,
}

struct VirtualFile {
    name: String,
    content: String,
    is_image: bool,
}

struct OSKernel {
    vram: Vec<u8>,
    windows: Vec<WindowState>,
    files: Vec<VirtualFile>,
    selected_file_idx: Option<usize>,
    is_editing_file: bool,        // ファイル書き換えモードのフラグ
    drag_offset_x: i32,
    drag_offset_y: i32,
    mouse_x: i32,
    mouse_y: i32,
    active_drag_id: Option<u8>,
    active_resize_id: Option<u8>,
    paint_pixels: Vec<u8>,
    paint_width: i32,
    paint_height: i32,
    brush_value: u8,              // ペイントの筆の濃さ（グレースケール）
    is_mouse_down: bool,
    cmd_buffer: String,
    terminal_logs: Vec<String>,
    cpu_usage_dummy: u8,          // タスクマネージャー用の演出統計
    draw_counter: u32,
    boot_ms: f64,                 // 起動時刻（js_sys::Date::now）
    storage_loaded: bool,
}

fn default_files() -> Vec<VirtualFile> {
    vec![
        VirtualFile { name: "README.TXT".to_string(), content: "TYPE HELP IN TERMINAL!".to_string(), is_image: false },
        VirtualFile { name: "NOTES.TXT".to_string(), content: "WASM OS WRITING SYSTEM ACTIVE.".to_string(), is_image: false },
        VirtualFile { name: "LOGO.BMP".to_string(), content: "IMAGE_DATA".to_string(), is_image: true },
    ]
}

thread_local! {
    static KERNEL: RefCell<OSKernel> = RefCell::new(OSKernel {
        vram: vec![0; VRAM_SIZE],
        windows: vec![
            WindowState { id: 1, title: "WasmOS Terminal v2.0".to_string(), x: 60, y: 70, width: 780, height: 460, is_open: true },
            WindowState { id: 2, title: "VRAM Paint app Pro".to_string(), x: 900, y: 70, width: 780, height: 560, is_open: true },
            WindowState { id: 3, title: "File Manager & Notepad".to_string(), x: 140, y: 580, width: 640, height: 420, is_open: true },
            WindowState { id: 4, title: "Task Manager".to_string(), x: 1080, y: 680, width: 560, height: 330, is_open: true },
        ],
        files: default_files(),
        selected_file_idx: Some(0),
        is_editing_file: false,
        drag_offset_x: 0,
        drag_offset_y: 0,
        mouse_x: 0,
        mouse_y: 0,
        active_drag_id: None,
        active_resize_id: None,
        paint_pixels: vec![255; PAINT_STRIDE * PAINT_STRIDE],
        paint_width: PAINT_W,
        paint_height: PAINT_H,
        brush_value: 0,
        is_mouse_down: false,
        cmd_buffer: String::new(),
        terminal_logs: vec![
            "WASM OS [VERSION 2.0 - FULL HD 1920X1080]".to_string(),
            "FILES ARE SAVED IN YOUR BROWSER (LOCAL STORAGE).".to_string(),
            "TYPE 'HELP' TO LIST ALL COMMANDS.".to_string(),
        ],
        cpu_usage_dummy: 12,
        draw_counter: 0,
        boot_ms: 0.0,
        storage_loaded: false,
    });
}

// ---------------------------------------------------------------------
// 永続化（localStorage にファイルを保存する）
// ---------------------------------------------------------------------
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn save_files(files: &[VirtualFile]) {
    let Some(storage) = local_storage() else { return };
    // 区切り文字は制御文字。名前・内容側は \ をエスケープしておく
    let mut out = String::new();
    for f in files {
        let name = f.name.replace('\\', "\\\\").replace('\u{1}', "");
        let content = f.content.replace('\\', "\\\\").replace('\u{1}', "").replace('\u{2}', "");
        out.push_str(&name);
        out.push('\u{1}');
        out.push_str(&content);
        out.push('\u{1}');
        out.push(if f.is_image { '1' } else { '0' });
        out.push('\u{2}');
    }
    let _ = storage.set_item(STORAGE_KEY, &out);
}

fn load_files() -> Option<Vec<VirtualFile>> {
    let storage = local_storage()?;
    let raw = storage.get_item(STORAGE_KEY).ok()??;
    let mut files = Vec::new();
    for record in raw.split('\u{2}') {
        if record.is_empty() { continue; }
        let parts: Vec<&str> = record.split('\u{1}').collect();
        if parts.len() != 3 { continue; }
        let unescape = |s: &str| s.replace("\\\\", "\\");
        files.push(VirtualFile {
            name: unescape(parts[0]),
            content: unescape(parts[1]),
            is_image: parts[2] == "1",
        });
    }
    if files.is_empty() { None } else { Some(files) }
}

// ---------------------------------------------------------------------
// フォント（3x5 ドット）。数字は1文字ずつ個別の字形、記号も一通り用意。
// ---------------------------------------------------------------------
fn get_font_pattern(c: char) -> [u8; 15] {
    match c.to_ascii_uppercase() {
        'A' => [1,1,1, 1,0,1, 1,1,1, 1,0,1, 1,0,1], 'B' => [1,1,0, 1,0,1, 1,1,0, 1,0,1, 1,1,0],
        'C' => [1,1,1, 1,0,0, 1,0,0, 1,0,0, 1,1,1], 'D' => [1,1,0, 1,0,1, 1,0,1, 1,0,1, 1,1,0],
        'E' => [1,1,1, 1,0,0, 1,1,1, 1,0,0, 1,1,1], 'F' => [1,1,1, 1,0,0, 1,1,1, 1,0,0, 1,0,0],
        'G' => [1,1,1, 1,0,0, 1,0,1, 1,0,1, 1,1,1], 'H' => [1,0,1, 1,0,1, 1,1,1, 1,0,1, 1,0,1],
        'I' => [1,1,1, 0,1,0, 0,1,0, 0,1,0, 1,1,1], 'J' => [0,0,1, 0,0,1, 0,0,1, 1,0,1, 1,1,1],
        'K' => [1,0,1, 1,0,1, 1,1,0, 1,0,1, 1,0,1], 'L' => [1,0,0, 1,0,0, 1,0,0, 1,0,0, 1,1,1],
        'M' => [1,0,1, 1,1,1, 1,0,1, 1,0,1, 1,0,1], 'N' => [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,0,1],
        'O' => [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], 'P' => [1,1,1, 1,0,1, 1,1,1, 1,0,0, 1,0,0],
        'Q' => [1,1,1, 1,0,1, 1,1,1, 0,0,1, 0,0,1], 'R' => [1,1,1, 1,0,1, 1,1,0, 1,0,1, 1,0,1],
        'S' => [1,1,1, 1,0,0, 1,1,1, 0,0,1, 1,1,1], 'T' => [1,1,1, 0,1,0, 0,1,0, 0,1,0, 0,1,0],
        'U' => [1,0,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], 'V' => [1,0,1, 1,0,1, 1,0,1, 1,0,1, 0,1,0],
        'W' => [1,0,1, 1,0,1, 1,0,1, 1,1,1, 1,0,1], 'X' => [1,0,1, 1,0,1, 0,1,0, 1,0,1, 1,0,1],
        'Y' => [1,0,1, 1,0,1, 1,1,1, 0,1,0, 0,1,0], 'Z' => [1,1,1, 0,0,1, 0,1,0, 1,0,0, 1,1,1],
        '0' => [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1], '1' => [0,1,0, 1,1,0, 0,1,0, 0,1,0, 1,1,1],
        '2' => [1,1,1, 0,0,1, 1,1,1, 1,0,0, 1,1,1], '3' => [1,1,1, 0,0,1, 0,1,1, 0,0,1, 1,1,1],
        '4' => [1,0,1, 1,0,1, 1,1,1, 0,0,1, 0,0,1], '5' => [1,1,1, 1,0,0, 1,1,1, 0,0,1, 1,1,1],
        '6' => [1,1,1, 1,0,0, 1,1,1, 1,0,1, 1,1,1], '7' => [1,1,1, 0,0,1, 0,0,1, 0,1,0, 0,1,0],
        '8' => [1,1,1, 1,0,1, 1,1,1, 1,0,1, 1,1,1], '9' => [1,1,1, 1,0,1, 1,1,1, 0,0,1, 1,1,1],
        '-' => [0,0,0, 0,0,0, 1,1,1, 0,0,0, 0,0,0], '%' => [1,0,1, 0,0,1, 0,1,0, 1,0,0, 1,0,1],
        '.' => [0,0,0, 0,0,0, 0,0,0, 0,0,0, 0,1,0], ':' => [0,0,0, 0,1,0, 0,0,0, 0,1,0, 0,0,0],
        '>' => [1,0,0, 0,1,0, 0,0,1, 0,1,0, 1,0,0], '[' => [1,1,1, 1,0,0, 1,0,0, 1,0,0, 1,1,1],
        ']' => [1,1,1, 0,0,1, 0,0,1, 0,0,1, 1,1,1], '+' => [0,0,0, 0,1,0, 1,1,1, 0,1,0, 0,0,0],
        '=' => [0,0,0, 1,1,1, 0,0,0, 1,1,1, 0,0,0], '/' => [0,0,1, 0,0,1, 0,1,0, 1,0,0, 1,0,0],
        '\\' => [1,0,0, 1,0,0, 0,1,0, 0,0,1, 0,0,1], '_' => [0,0,0, 0,0,0, 0,0,0, 0,0,0, 1,1,1],
        '(' => [0,0,1, 0,1,0, 0,1,0, 0,1,0, 0,0,1], ')' => [1,0,0, 0,1,0, 0,1,0, 0,1,0, 1,0,0],
        '!' => [0,1,0, 0,1,0, 0,1,0, 0,0,0, 0,1,0], '?' => [1,1,1, 0,0,1, 0,1,1, 0,0,0, 0,1,0],
        ',' => [0,0,0, 0,0,0, 0,0,0, 0,1,0, 1,0,0], '\'' => [0,1,0, 0,1,0, 0,0,0, 0,0,0, 0,0,0],
        '"' => [1,0,1, 1,0,1, 0,0,0, 0,0,0, 0,0,0], '<' => [0,0,1, 0,1,0, 1,0,0, 0,1,0, 0,0,1],
        '*' => [0,0,0, 1,0,1, 0,1,0, 1,0,1, 0,0,0], '#' => [1,0,1, 1,1,1, 1,0,1, 1,1,1, 1,0,1],
        '$' => [0,1,0, 1,1,1, 1,1,0, 0,1,1, 1,1,0], '&' => [0,1,0, 1,0,1, 0,1,0, 1,0,1, 0,1,1],
        '@' => [1,1,1, 1,0,1, 1,0,1, 1,0,0, 1,1,1], ';' => [0,0,0, 0,1,0, 0,0,0, 0,1,0, 1,0,0],
        '^' => [0,1,0, 1,0,1, 0,0,0, 0,0,0, 0,0,0], '~' => [0,0,0, 0,0,0, 0,1,1, 1,1,0, 0,0,0],
        '|' => [0,1,0, 0,1,0, 0,1,0, 0,1,0, 0,1,0], '{' => [0,1,1, 0,1,0, 1,1,0, 0,1,0, 0,1,1],
        '}' => [1,1,0, 0,1,0, 0,1,1, 0,1,0, 1,1,0], '`' => [1,0,0, 0,1,0, 0,0,0, 0,0,0, 0,0,0],
        _   => [0,0,0, 0,0,0, 0,0,0, 0,0,0, 0,0,0],
    }
}

fn draw_char_scaled(vram: &mut [u8], x: i32, y: i32, c: char, r: u8, g: u8, b: u8) {
    let pattern = get_font_pattern(c);
    for row in 0..5 {
        for col in 0..3 {
            if pattern[row * 3 + col] == 1 {
                let c_i32 = col as i32; let r_i32 = row as i32;
                set_pixel(vram, x + (c_i32 * 2),     y + (r_i32 * 2),     r, g, b);
                set_pixel(vram, x + (c_i32 * 2) + 1, y + (r_i32 * 2),     r, g, b);
                set_pixel(vram, x + (c_i32 * 2),     y + (r_i32 * 2) + 1, r, g, b);
                set_pixel(vram, x + (c_i32 * 2) + 1, y + (r_i32 * 2) + 1, r, g, b);
            }
        }
    }
}

fn draw_string_scaled(vram: &mut [u8], x: i32, y: i32, text: &str, r: u8, g: u8, b: u8) {
    let mut cur_x = x;
    for c in text.chars() { draw_char_scaled(vram, cur_x, y, c, r, g, b); cur_x += 9; }
}

fn text_width(text: &str) -> i32 { text.chars().count() as i32 * 9 }

#[wasm_bindgen]
pub fn get_vram_ptr() -> *const u8 { KERNEL.with(|k| k.borrow().vram.as_ptr()) }

#[wasm_bindgen]
pub fn init_kernel() {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        if !k.storage_loaded {
            k.storage_loaded = true;
            k.boot_ms = js_sys::Date::now();
            if let Some(files) = load_files() {
                k.files = files;
                k.selected_file_idx = if k.files.is_empty() { None } else { Some(0) };
            }
        }
    });
    render_all();
}

fn set_pixel(vram: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8) {
    if x < 0 || x >= WIDTH as i32 || y < 0 || y >= HEIGHT as i32 { return; }
    let idx = ((y as usize) * WIDTH + (x as usize)) * 4;
    vram[idx] = r; vram[idx + 1] = g; vram[idx + 2] = b; vram[idx + 3] = 255;
}

fn clear_vram(vram: &mut [u8], r: u8, g: u8, b: u8) {
    for i in (0..VRAM_SIZE).step_by(4) { vram[i] = r; vram[i + 1] = g; vram[i + 2] = b; vram[i + 3] = 255; }
}

fn fill_rect(vram: &mut [u8], x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8) {
    for py in y..(y + h) { for px in x..(x + w) { set_pixel(vram, px, py, r, g, b); } }
}

fn draw_window_high_quality(vram: &mut [u8], win: &WindowState, kernel: &OSKernel) {
    if !win.is_open { return; }

    fill_rect(vram, win.x, win.y, win.width, win.height, 0xd4, 0xd0, 0xc8);
    for px in win.x..(win.x + win.width) {
        set_pixel(vram, px, win.y, 255, 255, 255); set_pixel(vram, px, win.y + win.height - 1, 0x40, 0x40, 0x40);
    }
    for py in win.y..(win.y + win.height) {
        set_pixel(vram, win.x, py, 255, 255, 255); set_pixel(vram, win.x + win.width - 1, py, 0x40, 0x40, 0x40);
    }
    for py in (win.y + 3)..(win.y + 28) {
        let t = (py - (win.y + 3)) as f32 / 25.0;
        let r = (0x00 as f32 * (1.0 - t) + 0x0a as f32 * t) as u8;
        let g = (0x00 as f32 * (1.0 - t) + 0x5a as f32 * t) as u8;
        let b = (0x80 as f32 * (1.0 - t) + 0xff as f32 * t) as u8;
        for px in (win.x + 4)..(win.x + win.width - 4) { set_pixel(vram, px, py, r, g, b); }
    }
    draw_string_scaled(vram, win.x + 12, win.y + 8, &win.title, 255, 255, 255);

    let bx = win.x + win.width - 24; let by = win.y + 6;
    fill_rect(vram, bx, by, 16, 16, 0xe0, 0x40, 0x40);
    draw_string_scaled(vram, bx + 5, by + 3, "X", 255, 255, 255);

    let rx = win.x + win.width - 12; let ry = win.y + win.height - 12;
    for i in 0..10 { for j in i..10 { if (i + j) % 3 == 0 { set_pixel(vram, rx + i, ry + j, 0x80, 0x80, 0x80); } } }

    if win.id == 1 {
        // 1. ターミナル（折り返し＋末尾スクロール表示）
        let tx = win.x + 8; let ty = win.y + 34; let tw = win.width - 16; let th = win.height - 42;
        if tw > 20 && th > 40 {
            fill_rect(vram, tx, ty, tw, th, 0x05, 0x05, 0x05);
            let chars_per_line = (((tw - 24) / 9).max(1)) as usize;
            let max_lines = (((th - 34) / 16).max(1)) as usize;
            // ログを画面幅で折り返してから、末尾 max_lines 行だけを見せる
            let mut visual: Vec<String> = Vec::new();
            for log in &kernel.terminal_logs {
                if log.is_empty() { visual.push(String::new()); continue; }
                let chars: Vec<char> = log.chars().collect();
                let mut start = 0;
                while start < chars.len() {
                    let end = (start + chars_per_line).min(chars.len());
                    visual.push(chars[start..end].iter().collect());
                    start = end;
                }
            }
            let skip = visual.len().saturating_sub(max_lines);
            let mut line_y = ty + 8;
            for line in &visual[skip..] {
                draw_string_scaled(vram, tx + 12, line_y, line, 0x33, 0xff, 0x33);
                line_y += 16;
            }
            let blink = if (kernel.draw_counter / 30) % 2 == 0 { "_" } else { " " };
            let prompt = format!(">{}{}", kernel.cmd_buffer, if kernel.is_editing_file { "" } else { blink });
            draw_string_scaled(vram, tx + 12, ty + th - 20, &prompt, 255, 255, 255);
        }
    } else if win.id == 2 {
        // 2. ペイント（キャンバス＋下部にグレーのパレット）
        let cx = win.x + 10; let cy = win.y + 34; let cw = win.width - 20; let ch = win.height - 45;
        if cw > 10 && ch > 10 {
            for py in cy..(cy + ch) {
                for px in cx..(cx + cw) {
                    let dx = px - cx; let dy = py - cy;
                    if dx >= 0 && dx < kernel.paint_width && dy >= 0 && dy < kernel.paint_height {
                        let p_color = kernel.paint_pixels[(dy as usize) * PAINT_STRIDE + dx as usize];
                        set_pixel(vram, px, py, p_color, p_color, p_color);
                    } else { set_pixel(vram, px, py, 0xaa, 0xaa, 0xaa); }
                }
            }
            // パレット（クリックで筆の濃さを切り替え）
            let pal_y = cy + kernel.paint_height + 10;
            if pal_y + 16 < cy + ch {
                for (i, value) in PALETTE.iter().enumerate() {
                    let sx = cx + (i as i32) * 24;
                    fill_rect(vram, sx, pal_y, 18, 16, *value, *value, *value);
                    if *value == kernel.brush_value {
                        for px in sx..(sx + 18) { set_pixel(vram, px, pal_y - 2, 0xff, 0x33, 0x33); }
                    }
                }
                let label = format!("BRUSH: {}", kernel.brush_value);
                draw_string_scaled(vram, cx + 8 * 24 + 12, pal_y + 3, &label, 0, 0, 0);
            }
        }
    } else if win.id == 3 {
        // 3. File Manager & Notepad（ファイル一覧 ＋ テキスト編集UI）
        let fx = win.x + 10; let fy = win.y + 34;
        let fw = win.width - 20; let fh = win.height - 85; // 下部に編集用領域を確保
        if fw > 10 && fh > 10 {
            fill_rect(vram, fx, fy, fw, fh, 255, 255, 255);
            let mut item_y = fy + 6;
            for (idx, file) in kernel.files.iter().enumerate() {
                if item_y + 20 > fy + fh { break; }
                if Some(idx) == kernel.selected_file_idx {
                    fill_rect(vram, fx + 4, item_y, fw - 8, 18, 0x00, 0x00, 0x80);
                }
                let icon = if file.is_image { "[I]" } else { "[T]" };
                let t_color = if Some(idx) == kernel.selected_file_idx { (255, 255, 255) } else { (0, 0, 0) };
                draw_string_scaled(vram, fx + 10, item_y + 2, icon, 0xff, 0xaa, 0x00);
                draw_string_scaled(vram, fx + 40, item_y + 2, &file.name, t_color.0, t_color.1, t_color.2);
                item_y += 22;
            }

            // 下部のメモ帳テキスト編集エディタ
            let ex = fx; let ey = fy + fh + 5; let ew = fw; let eh = 40;
            fill_rect(vram, ex, ey, ew, eh, 0xf5, 0xf5, 0xf5);

            // ボタンの描画
            let btn_x = ex + ew - 65; let btn_y = ey + 10;
            let btn_color = if kernel.is_editing_file { 0xff } else { 0xaa };
            fill_rect(vram, btn_x, btn_y, 55, 20, 0x44, btn_color, 0x44);
            draw_string_scaled(vram, btn_x + 8, btn_y + 4, "EDIT", 255, 255, 255);

            // 現在選択されているファイルの中身を表示（編集中は末尾が見えるよう切り取る）
            if let Some(idx) = kernel.selected_file_idx {
                if idx < kernel.files.len() {
                    let full = if kernel.is_editing_file {
                        format!("{}_", kernel.cmd_buffer)
                    } else {
                        kernel.files[idx].content.clone()
                    };
                    let fit = (((ew - 90) / 9).max(1)) as usize;
                    let display_text: String = {
                        let chars: Vec<char> = full.chars().collect();
                        if chars.len() > fit { chars[chars.len() - fit..].iter().collect() } else { full }
                    };
                    draw_string_scaled(vram, ex + 10, ey + 14, &display_text, 0, 0, 0);
                }
            }
        }
    } else if win.id == 4 {
        // 4. Task Manager（実状態のモニター）
        let mx = win.x + 10; let my = win.y + 34; let mw = win.width - 20; let mh = win.height - 45;
        if mw > 10 && mh > 10 {
            fill_rect(vram, mx, my, mw, mh, 0x10, 0x20, 0x10);
            draw_string_scaled(vram, mx + 15, my + 12, "SYSTEM PERFORMANCE", 0x33, 0xff, 0x33);

            let uptime_sec = ((js_sys::Date::now() - kernel.boot_ms) / 1000.0).max(0.0) as u64;
            let open_count = kernel.windows.iter().filter(|w| w.is_open).count();
            let total_bytes: usize = kernel.files.iter().map(|f| f.content.len()).sum();
            let stats = [
                format!("UPTIME     : {} SEC", uptime_sec),
                format!("CPU (SIM)  : {}%", kernel.cpu_usage_dummy),
                format!("WINDOWS    : {} / {} OPEN", open_count, kernel.windows.len()),
                format!("FILES      : {} ({} BYTES)", kernel.files.len(), total_bytes),
                format!("MOUSE      : {}, {}", kernel.mouse_x, kernel.mouse_y),
                format!("FRAMES     : {}", kernel.draw_counter),
                format!("DISPLAY    : {}X{} (VRAM {:.1} MB)", WIDTH, HEIGHT,
                        (VRAM_SIZE as f64) / 1_048_576.0),
            ];
            let mut line_y = my + 36;
            for line in &stats {
                draw_string_scaled(vram, mx + 15, line_y, line, 255, 255, 255);
                line_y += 17;
            }

            // パルスグラフの描画
            let gx = mx + 15; let gy = line_y + 6; let gw = mw - 30; let gh = (my + mh) - gy - 10;
            if gw > 20 && gh > 20 {
                for px in gx..(gx + gw) {
                    set_pixel(vram, px, gy + gh - 1, 0, 0x88, 0); // ベースライン
                }
                for px in gx..(gx + gw) {
                    let offset = (px - gx) as f32;
                    let wave = (((offset + kernel.draw_counter as f32) * 0.1).sin() * (gh as f32 * 0.3)) as i32;
                    let py = gy + (gh / 2) + wave;
                    if py >= gy && py < gy + gh { set_pixel(vram, px, py, 0x33, 0xff, 0x33); }
                }
            }
        }
    }
}

// タスクバーのボタン定義（ラベル, x1, x2, 色, 対応ウィンドウ index）
const TASKBAR_BUTTONS: [(&str, i32, i32, (u8, u8, u8), usize); 4] = [
    ("TERM", 15, 115, (0x00, 0x00, 0x80), 0),
    ("DRAW", 130, 230, (0xff, 0xaa, 0x00), 1),
    ("FILE", 245, 345, (0x00, 0xaa, 0x55), 2),
    ("TASK", 360, 460, (0x55, 0x55, 0x55), 3),
];

fn draw_desktop_ui_high_quality(vram: &mut [u8]) {
    let bar_y = HEIGHT as i32 - TASKBAR_H;
    set_pixel(vram, 0, bar_y, 255, 255, 255);
    for px in 0..WIDTH as i32 { set_pixel(vram, px, bar_y, 255, 255, 255); set_pixel(vram, px, bar_y + 1, 0x80, 0x80, 0x80); }
    fill_rect(vram, 0, bar_y + 2, WIDTH as i32, TASKBAR_H - 2, 0xd4, 0xd0, 0xc8);

    for (label, x1, x2, color, _) in &TASKBAR_BUTTONS {
        fill_rect(vram, *x1, bar_y + 6, x2 - x1, 22, color.0, color.1, color.2);
        draw_string_scaled(vram, x1 + 10, bar_y + 12, label, 255, 255, 255);
    }
    // 右端に解像度表示
    let res = format!("{}X{}", WIDTH, HEIGHT);
    draw_string_scaled(vram, WIDTH as i32 - text_width(&res) - 16, bar_y + 12, &res, 0x33, 0x33, 0x33);
}

fn draw_mouse_cursor_hd(vram: &mut [u8], mx: i32, my: i32) {
    for dy in 0..18 {
        for dx in 0..dy {
            if dx == 0 || dx == dy - 1 || dy == 17 { set_pixel(vram, mx + dx, my + dy, 0, 0, 0); }
            else { set_pixel(vram, mx + dx, my + dy, 255, 255, 255); }
        }
    }
}

fn render_all() {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        k.draw_counter = k.draw_counter.wrapping_add(1);
        if k.draw_counter % 20 == 0 {
            k.cpu_usage_dummy = (10 + (k.draw_counter % 35) as u8) as u8; // 動的なCPU使用率演出
        }

        let mut vram = std::mem::take(&mut k.vram);
        clear_vram(&mut vram, 0x1a, 0x5c, 0x5a);

        let windows = std::mem::take(&mut k.windows);
        for win in &windows { draw_window_high_quality(&mut vram, win, &k); }
        k.windows = windows;

        draw_desktop_ui_high_quality(&mut vram);
        draw_mouse_cursor_hd(&mut vram, k.mouse_x, k.mouse_y);
        k.vram = vram;
    });
}

// ---------------------------------------------------------------------
// ターミナル命令
// ---------------------------------------------------------------------
fn now_string() -> String {
    let d = js_sys::Date::new_0();
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            d.get_full_year(), d.get_month() + 1, d.get_date(),
            d.get_hours(), d.get_minutes(), d.get_seconds())
}

fn valid_file_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 24
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 簡単な四則演算（+ - * / と括弧）の評価器
struct CalcParser { chars: Vec<char>, pos: usize }
impl CalcParser {
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn next(&mut self) -> Option<char> { let c = self.peek(); if c.is_some() { self.pos += 1; } c }
    fn expr(&mut self) -> Option<f64> {
        let mut value = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.next(); value += self.term()?; }
                Some('-') => { self.next(); value -= self.term()?; }
                _ => break,
            }
        }
        Some(value)
    }
    fn term(&mut self) -> Option<f64> {
        let mut value = self.factor()?;
        loop {
            match self.peek() {
                Some('*') => { self.next(); value *= self.factor()?; }
                Some('/') => {
                    self.next();
                    let divisor = self.factor()?;
                    if divisor == 0.0 { return None; }
                    value /= divisor;
                }
                _ => break,
            }
        }
        Some(value)
    }
    fn factor(&mut self) -> Option<f64> {
        match self.peek()? {
            '-' => { self.next(); Some(-self.factor()?) }
            '(' => {
                self.next();
                let value = self.expr()?;
                if self.next() != Some(')') { return None; }
                Some(value)
            }
            _ => {
                let mut text = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '.' { text.push(c); self.next(); } else { break; }
                }
                text.parse::<f64>().ok()
            }
        }
    }
}

fn calc(expression: &str) -> Option<f64> {
    let mut parser = CalcParser {
        chars: expression.chars().filter(|c| !c.is_whitespace()).collect(),
        pos: 0,
    };
    let value = parser.expr()?;
    if parser.pos != parser.chars.len() { return None; }
    Some(value)
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let text = format!("{:.6}", value);
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn app_index(name: &str) -> Option<usize> {
    match name {
        "term" | "terminal" => Some(0),
        "paint" | "draw" => Some(1),
        "file" | "files" => Some(2),
        "task" | "taskmgr" => Some(3),
        _ => None,
    }
}

fn execute_command(k: &mut OSKernel, raw: &str) {
    let trimmed = raw.trim();
    k.terminal_logs.push(format!(">{}", trimmed));
    if trimmed.is_empty() { return; }
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("").to_lowercase();
    let rest = parts.next().unwrap_or("").trim();
    let rest_upper = rest.to_uppercase();

    match cmd.as_str() {
        "help" => {
            for line in [
                "HELP           : SHOW THIS LIST",
                "CLEAR / CLS    : CLEAR SCREEN",
                "VER            : SHOW VERSION",
                "ECHO [TEXT]    : PRINT TEXT",
                "LS / DIR       : LIST FILES",
                "CAT [FILE]     : READ FILE",
                "TOUCH [NAME]   : CREATE EMPTY FILE",
                "EDIT [FILE]    : EDIT FILE IN NOTEPAD",
                "RM [FILE]      : DELETE FILE",
                "MV [OLD] [NEW] : RENAME FILE",
                "CALC [EXPR]    : CALCULATE (EX: CALC 12*(3+4))",
                "TIME / DATE    : SHOW CLOCK",
                "OPEN [APP]     : OPEN APP (TERM/PAINT/FILE/TASK)",
                "CLOSE [APP]    : CLOSE APP",
                "PS             : LIST WINDOWS",
                "MEM            : SYSTEM INFO",
                "PAINT          : RESET CANVAS",
                "BRUSH [0-255]  : SET PAINT BRUSH GRAY",
                "ABOUT          : ABOUT WASM OS",
                "EXIT           : CLOSE TERMINAL",
            ] { k.terminal_logs.push(line.to_string()); }
        }
        "clear" | "cls" => k.terminal_logs.clear(),
        "ver" => k.terminal_logs.push(format!(
            "WASM OS VERSION {} - FULL HD {}X{}", VERSION, WIDTH, HEIGHT)),
        "echo" => k.terminal_logs.push(rest.to_string()),
        "ls" | "dir" => {
            if k.files.is_empty() {
                k.terminal_logs.push("NO FILES".to_string());
            }
            for f in &k.files {
                let kind = if f.is_image { "IMG" } else { "TXT" };
                k.terminal_logs.push(format!("{} [{}] {} BYTES", f.name, kind, f.content.len()));
            }
        }
        "cat" => {
            match k.files.iter().find(|f| f.name == rest_upper) {
                Some(f) if f.is_image => k.terminal_logs.push("BINARY IMAGE DATA".to_string()),
                Some(f) => k.terminal_logs.push(f.content.clone()),
                None => k.terminal_logs.push("FILE NOT FOUND".to_string()),
            }
        }
        "touch" | "create" => {
            let name = rest_upper.clone();
            if !valid_file_name(&name) {
                k.terminal_logs.push("BAD FILE NAME (A-Z 0-9 . _ -, MAX 24)".to_string());
            } else if k.files.iter().any(|f| f.name == name) {
                k.terminal_logs.push("FILE ALREADY EXISTS".to_string());
            } else {
                k.files.push(VirtualFile { name: name.clone(), content: String::new(), is_image: false });
                k.terminal_logs.push(format!("CREATED {}", name));
                save_files(&k.files);
            }
        }
        "edit" => {
            match k.files.iter().position(|f| f.name == rest_upper) {
                Some(idx) if !k.files[idx].is_image => {
                    k.selected_file_idx = Some(idx);
                    k.is_editing_file = true;
                    k.cmd_buffer = k.files[idx].content.clone();
                    k.windows[2].is_open = true;
                    k.terminal_logs.push(format!("EDITING {} - ENTER TO SAVE", k.files[idx].name));
                }
                Some(_) => k.terminal_logs.push("CANNOT EDIT IMAGE FILE".to_string()),
                None => k.terminal_logs.push("FILE NOT FOUND".to_string()),
            }
        }
        "rm" | "del" | "delete" => {
            match k.files.iter().position(|f| f.name == rest_upper) {
                Some(idx) => {
                    let removed = k.files.remove(idx);
                    if k.selected_file_idx == Some(idx) {
                        k.selected_file_idx = if k.files.is_empty() { None } else { Some(0) };
                    }
                    k.terminal_logs.push(format!("DELETED {}", removed.name));
                    save_files(&k.files);
                }
                None => k.terminal_logs.push("FILE NOT FOUND".to_string()),
            }
        }
        "mv" | "rename" => {
            let args: Vec<&str> = rest.split_whitespace().collect();
            if args.len() != 2 {
                k.terminal_logs.push("USAGE: MV [OLD] [NEW]".to_string());
            } else {
                let old = args[0].to_uppercase();
                let new = args[1].to_uppercase();
                if !valid_file_name(&new) {
                    k.terminal_logs.push("BAD FILE NAME".to_string());
                } else if k.files.iter().any(|f| f.name == new) {
                    k.terminal_logs.push("TARGET ALREADY EXISTS".to_string());
                } else if let Some(f) = k.files.iter_mut().find(|f| f.name == old) {
                    f.name = new.clone();
                    k.terminal_logs.push(format!("RENAMED {} TO {}", old, new));
                    save_files(&k.files);
                } else {
                    k.terminal_logs.push("FILE NOT FOUND".to_string());
                }
            }
        }
        "calc" => {
            match calc(rest) {
                Some(value) => k.terminal_logs.push(format!("= {}", format_number(value))),
                None => k.terminal_logs.push("CALC ERROR (EX: CALC 12*(3+4))".to_string()),
            }
        }
        "time" | "date" | "clock" => k.terminal_logs.push(now_string()),
        "open" => {
            match app_index(&rest.to_lowercase()) {
                Some(idx) => {
                    k.windows[idx].is_open = true;
                    k.terminal_logs.push(format!("OPENED {}", rest_upper));
                }
                None => k.terminal_logs.push("APP NOT FOUND (TERM/PAINT/FILE/TASK)".to_string()),
            }
        }
        "close" => {
            match app_index(&rest.to_lowercase()) {
                Some(idx) => {
                    k.windows[idx].is_open = false;
                    k.terminal_logs.push(format!("CLOSED {}", rest_upper));
                }
                None => k.terminal_logs.push("APP NOT FOUND (TERM/PAINT/FILE/TASK)".to_string()),
            }
        }
        "ps" => {
            for w in &k.windows {
                let state = if w.is_open { "OPEN  " } else { "CLOSED" };
                k.terminal_logs.push(format!("[{}] {} ({}X{})", state, w.title, w.width, w.height));
            }
        }
        "mem" => {
            let total_bytes: usize = k.files.iter().map(|f| f.content.len()).sum();
            k.terminal_logs.push(format!("VRAM: {:.1} MB ({}X{})", (VRAM_SIZE as f64) / 1_048_576.0, WIDTH, HEIGHT));
            k.terminal_logs.push(format!("FILES: {} ({} BYTES)", k.files.len(), total_bytes));
            let uptime_sec = ((js_sys::Date::now() - k.boot_ms) / 1000.0).max(0.0) as u64;
            k.terminal_logs.push(format!("UPTIME: {} SEC / FRAMES: {}", uptime_sec, k.draw_counter));
        }
        "paint" => {
            k.paint_pixels = vec![255; PAINT_STRIDE * PAINT_STRIDE];
            k.terminal_logs.push("CANVAS RESET".to_string());
        }
        "brush" => {
            match rest.parse::<u8>() {
                Ok(value) => {
                    k.brush_value = value;
                    k.terminal_logs.push(format!("BRUSH SET TO {}", value));
                }
                Err(_) => k.terminal_logs.push(format!("BRUSH IS {} (USAGE: BRUSH 0-255)", k.brush_value)),
            }
        }
        "about" => {
            k.terminal_logs.push("WASM OS - A TINY DESKTOP OS IN RUST + WEBASSEMBLY.".to_string());
            k.terminal_logs.push("FILES PERSIST IN LOCAL STORAGE.".to_string());
        }
        "exit" => {
            k.windows[0].is_open = false;
        }
        _ => k.terminal_logs.push("COMMAND NOT FOUND. TYPE 'HELP'.".to_string()),
    }
    // ログが際限なく伸びないよう上限を設ける
    if k.terminal_logs.len() > 200 {
        let excess = k.terminal_logs.len() - 200;
        k.terminal_logs.drain(..excess);
    }
}

#[wasm_bindgen]
pub fn on_os_keypress(key: &str) {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();

        // ファイル書き換え中の入力はメモ帳のバッファへ流れる
        if k.is_editing_file {
            if key == "Enter" {
                if let Some(idx) = k.selected_file_idx {
                    if idx < k.files.len() {
                        k.files[idx].content = k.cmd_buffer.clone();
                        let name = k.files[idx].name.clone();
                        save_files(&k.files);
                        k.terminal_logs.push(format!("SAVED TO {}", name));
                    }
                }
                k.cmd_buffer.clear();
                k.is_editing_file = false;
            } else if key == "Backspace" {
                k.cmd_buffer.pop();
            } else if key == "Escape" {
                k.cmd_buffer.clear();
                k.is_editing_file = false;
                k.terminal_logs.push("EDIT CANCELLED".to_string());
            } else if key.len() == 1 {
                let c = key.chars().next().unwrap();
                if c.is_ascii() && !c.is_ascii_control() && k.cmd_buffer.len() < 120 {
                    k.cmd_buffer.push(c);
                }
            }
            return;
        }

        if !k.windows[0].is_open { return; }
        if key == "Enter" {
            let raw = k.cmd_buffer.clone();
            k.cmd_buffer.clear();
            execute_command(&mut k, &raw);
        } else if key == "Backspace" {
            k.cmd_buffer.pop();
        } else if key.len() == 1 {
            let c = key.chars().next().unwrap();
            if c.is_ascii() && !c.is_ascii_control() && k.cmd_buffer.len() < 96 {
                k.cmd_buffer.push(c);
            }
        }
    });
    render_all();
}

#[wasm_bindgen]
pub fn on_mouse_down(mx: f64, my: f64) {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        let mx = mx as i32; let my = my as i32;
        k.is_mouse_down = true;

        // タスクバーのボタン
        let bar_y = HEIGHT as i32 - TASKBAR_H;
        if my >= bar_y + 6 && my <= bar_y + 28 {
            for (_, x1, x2, _, win_idx) in &TASKBAR_BUTTONS {
                if mx >= *x1 && mx <= *x2 {
                    if let Some(w) = k.windows.get_mut(*win_idx) { w.is_open = true; }
                }
            }
        }

        let mut clicked_close_id: Option<u8> = None;
        let mut clicked_resize_id: Option<u8> = None;
        let mut clicked_drag_id: Option<u8> = None;
        let mut drag_offset: (i32, i32) = (0, 0);
        let mut paint_pixel_idx: Option<usize> = None;
        let mut palette_value: Option<u8> = None;
        let mut file_click_idx: Option<usize> = None;
        let mut edit_btn_clicked = false;

        for win in k.windows.iter().rev() {
            if !win.is_open { continue; }

            if mx >= (win.x + win.width - 24) && mx <= (win.x + win.width - 8) && my >= (win.y + 6) && my <= (win.y + 22) {
                clicked_close_id = Some(win.id); break;
            }
            if mx >= (win.x + win.width - 16) && mx <= (win.x + win.width) && my >= (win.y + win.height - 16) && my <= (win.y + win.height) {
                clicked_resize_id = Some(win.id); drag_offset = (mx - win.width, my - win.height); break;
            }
            if mx >= win.x && mx <= (win.x + win.width) && my >= win.y && my <= (win.y + 30) {
                clicked_drag_id = Some(win.id); drag_offset = (mx - win.x, my - win.y); break;
            }
            if win.id == 2 {
                let cx = mx - (win.x + 10); let cy = my - (win.y + 34);
                if cx >= 0 && cx < k.paint_width && cy >= 0 && cy < k.paint_height {
                    paint_pixel_idx = Some((cy as usize) * PAINT_STRIDE + cx as usize); break;
                }
                // パレットのクリック
                let pal_y = k.paint_height + 10;
                if cy >= pal_y && cy < pal_y + 16 {
                    let idx = cx / 24;
                    if idx >= 0 && (idx as usize) < PALETTE.len() && cx % 24 < 18 {
                        palette_value = Some(PALETTE[idx as usize]); break;
                    }
                }
            }
            if win.id == 3 {
                let fx = mx - (win.x + 10); let fy = my - (win.y + 34);
                let fw = win.width - 20; let fh = win.height - 85;

                // EDITボタンのクリック判定
                let btn_x = fw - 65; let btn_y = fh + 10;
                if fx >= btn_x && fx <= btn_x + 55 && fy >= btn_y && fy <= btn_y + 20 {
                    edit_btn_clicked = true; break;
                }
                // ファイル行クリック
                if fx >= 0 && fx < fw && fy >= 0 && fy < fh {
                    let clicked_row = (fy - 8) / 24;
                    if clicked_row >= 0 && (clicked_row as usize) < k.files.len() { file_click_idx = Some(clicked_row as usize); break; }
                }
            }
        }

        if let Some(id) = clicked_close_id { if let Some(w) = k.windows.iter_mut().find(|w| w.id == id) { w.is_open = false; } }
        if let Some(id) = clicked_resize_id { k.active_resize_id = Some(id); k.drag_offset_x = drag_offset.0; k.drag_offset_y = drag_offset.1; }
        if let Some(id) = clicked_drag_id { k.active_drag_id = Some(id); k.drag_offset_x = drag_offset.0; k.drag_offset_y = drag_offset.1; }
        if let Some(idx) = paint_pixel_idx { let value = k.brush_value; k.paint_pixels[idx] = value; }
        if let Some(value) = palette_value { k.brush_value = value; }

        if file_click_idx.is_some() { k.is_editing_file = false; k.cmd_buffer.clear(); }

        if let Some(idx) = file_click_idx {
            k.selected_file_idx = Some(idx);
            let file_name = k.files[idx].name.clone();
            k.terminal_logs.push(format!("SELECTED: {}", file_name));
            if k.files[idx].is_image {
                for y in 30..90 { for x in 30..90 { k.paint_pixels[y * PAINT_STRIDE + x] = 0; } }
            }
        }

        // EDITボタンを押した時、入力バッファに現在のファイルテキストをロード
        if edit_btn_clicked {
            if let Some(idx) = k.selected_file_idx {
                if idx < k.files.len() && !k.files[idx].is_image {
                    k.is_editing_file = !k.is_editing_file;
                    if k.is_editing_file {
                        k.cmd_buffer = k.files[idx].content.clone();
                    } else {
                        k.cmd_buffer.clear();
                    }
                }
            }
        }
    });
    render_all();
}

#[wasm_bindgen]
pub fn on_mouse_move(mx: f64, my: f64) {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        let mx = mx as i32; let my = my as i32;
        k.mouse_x = mx; k.mouse_y = my;

        if let Some(id) = k.active_resize_id {
            let ox = k.drag_offset_x; let oy = k.drag_offset_y;
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) { win.width = (mx - ox).max(240); win.height = (my - oy).max(160); }
        }
        if let Some(id) = k.active_drag_id {
            let ox = k.drag_offset_x; let oy = k.drag_offset_y;
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) { win.x = mx - ox; win.y = my - oy; }
        }
        if k.is_mouse_down && k.active_drag_id.is_none() && k.active_resize_id.is_none() {
            if let Some(win) = k.windows.iter().find(|w| w.id == 2 && w.is_open) {
                let cx = mx - (win.x + 10); let cy = my - (win.y + 34);
                if cx >= 0 && cx < k.paint_width && cy >= 0 && cy < k.paint_height {
                    let value = k.brush_value;
                    k.paint_pixels[(cy as usize) * PAINT_STRIDE + cx as usize] = value;
                }
            }
        }
    });
    render_all();
}

#[wasm_bindgen]
pub fn on_mouse_up() {
    KERNEL.with(|k| { let mut k = k.borrow_mut(); k.active_drag_id = None; k.active_resize_id = None; k.is_mouse_down = false; });
    render_all();
}
