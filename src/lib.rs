use std::cell::RefCell;
use wasm_bindgen::prelude::*;

const WIDTH: usize = 1280;
const HEIGHT: usize = 720;
const VRAM_SIZE: usize = WIDTH * HEIGHT * 4;

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
    is_editing_file: bool,        // 👑 ファイル書き換えモードのフラグ
    drag_offset_x: i32,
    drag_offset_y: i32,
    mouse_x: i32,
    mouse_y: i32,
    active_drag_id: Option<u8>,
    active_resize_id: Option<u8>,
    paint_pixels: Vec<u8>, 
    paint_width: i32,             
    paint_height: i32,
    is_mouse_down: bool,
    cmd_buffer: String,
    terminal_logs: Vec<String>,
    cpu_usage_dummy: u8,          // 👑 タスクマネージャー用のダミー統計
    draw_counter: u32,
}

thread_local! {
    static KERNEL: RefCell<OSKernel> = RefCell::new(OSKernel {
        vram: vec![0; VRAM_SIZE],
        windows: vec![
            WindowState { id: 1, title: "WasmOS Terminal v1.3".to_string(), x: 40, y: 60, width: 400, height: 280, is_open: true },
            WindowState { id: 2, title: "VRAM Paint app Pro".to_string(), x: 460, y: 60, width: 400, height: 320, is_open: true },
            WindowState { id: 3, title: "File Manager & Notepad".to_string(), x: 100, y: 380, width: 340, height: 260, is_open: true },
            // 👑 4つ目の新アプリ「Task Manager」
            WindowState { id: 4, title: "Task Manager".to_string(), x: 460, y: 400, width: 280, height: 220, is_open: true },
        ],
        files: vec![
            VirtualFile { name: "README.TXT".to_string(), content: "EDIT THIS TEXT DIRECTLY!".to_string(), is_image: false },
            VirtualFile { name: "NOTES.TXT".to_string(), content: "WASM OS WRITING SYSTEM ACTIVE.".to_string(), is_image: false },
            VirtualFile { name: "LOGO.BMP".to_string(), content: "IMAGE_DATA".to_string(), is_image: true },
        ],
        selected_file_idx: Some(0),
        is_editing_file: false,
        drag_offset_x: 0,
        drag_offset_y: 0,
        mouse_x: 0,
        mouse_y: 0,
        active_drag_id: None,
        active_resize_id: None,
        paint_pixels: vec![255; 400 * 400], 
        paint_width: 200,
        paint_height: 200,
        is_mouse_down: false,
        cmd_buffer: String::new(),
        terminal_logs: vec![
            "WASM OS [Version 1.3.720p]".to_string(),
            "Notepad & Task Manager systems initialized.".to_string(),
            "Press EDIT button in File Manager to rewrite."
        ],
        cpu_usage_dummy: 12,
        draw_counter: 0,
    });
}

// フォントデータ
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
        '-' => [0,0,0, 0,0,0, 1,1,1, 0,0,0, 0,0,0], '%' => [1,0,1, 0,0,1, 0,1,0, 1,0,0, 1,0,1],
        '.' => [0,0,0, 0,0,0, 0,0,0, 0,0,0, 0,1,0], ':' => [0,0,0, 0,1,0, 0,0,0, 0,1,0, 0,0,0],
        '>' => [1,0,0, 0,1,0, 0,0,1, 0,1,0, 1,0,0], '[' => [1,1,1, 1,0,0, 1,0,0, 1,0,0, 1,1,1],
        ']' => [1,1,1, 0,0,1, 0,0,1, 0,0,1, 1,1,1], '0'..='9' => [1,1,1, 1,0,1, 1,0,1, 1,0,1, 1,1,1],
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

#[wasm_bindgen]
pub fn get_vram_ptr() -> *const u8 { KERNEL.with(|k| k.borrow().vram.as_ptr()) }

#[wasm_bindgen]
pub fn init_kernel() { render_all(); }

fn set_pixel(vram: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8) {
    if x < 0 || x >= WIDTH as i32 || y < 0 || y >= HEIGHT as i32 { return; }
    let idx = ((y as usize) * WIDTH + (x as usize)) * 4;
    vram[idx] = r; vram[idx + 1] = g; vram[idx + 2] = b; vram[idx + 3] = 255;
}

fn clear_vram(vram: &mut [u8], r: u8, g: u8, b: u8) {
    for i in (0..VRAM_SIZE).step_by(4) { vram[i] = r; vram[i + 1] = g; vram[i + 2] = b; vram[i + 3] = 255; }
}

fn draw_window_high_quality(vram: &mut [u8], win: &WindowState, kernel: &OSKernel) {
    if !win.is_open { return; }

    for py in win.y..(win.y + win.height) {
        for px in win.x..(win.x + win.width) { set_pixel(vram, px, py, 0xd4, 0xd0, 0xc8); }
    }
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
    for py in by..(by + 16) { for px in bx..(bx + 16) { set_pixel(vram, px, py, 0xe0, 0x40, 0x40); } }
    draw_string_scaled(vram, bx + 5, by + 3, "X", 255, 255, 255);

    let rx = win.x + win.width - 12; let ry = win.y + win.height - 12;
    for i in 0..10 { for j in i..10 { if (i + j) % 3 == 0 { set_pixel(vram, rx + i, ry + j, 0x80, 0x80, 0x80); } } }

    if win.id == 1 {
        // 1. ターミナル
        let tx = win.x + 8; let ty = win.y + 34; let tw = win.width - 16; let th = win.height - 42;
        if tw > 20 && th > 40 {
            for py in ty..(ty + th) { for px in tx..(tx + tw) { set_pixel(vram, px, py, 0x05, 0x05, 0x05); } }
            let mut line_y = ty + 10;
            for log in &kernel.terminal_logs {
                if line_y + 14 < ty + th - 25 { draw_string_scaled(vram, tx + 12, line_y, log, 0x33, 0xff, 0x33); line_y += 16; }
            }
            let prompt = format!(">{}", kernel.cmd_buffer);
            if ty + th - 20 > ty { draw_string_scaled(vram, tx + 12, ty + th - 20, &prompt, 255, 255, 255); }
        }
    } else if win.id == 2 {
        // 2. ペイント
        let cx = win.x + 10; let cy = win.y + 34; let cw = win.width - 20; let ch = win.height - 45;
        if cw > 10 && ch > 10 {
            for py in cy..(cy + ch) {
                for px in cx..(cx + cw) {
                    let dx = px - cx; let dy = py - cy;
                    if dx >= 0 && dx < kernel.paint_width && dy >= 0 && dy < kernel.paint_height {
                        let p_color = kernel.paint_pixels[(dy * 400 + dx) as usize]; set_pixel(vram, px, py, p_color, p_color, p_color);
                    } else { set_pixel(vram, px, py, 0xaa, 0xaa, 0xaa); }
                }
            }
        }
    } else if win.id == 3 {
        // 👑 3. File Manager & Notepad（ファイル一覧 ＋ テキスト編集UI）
        let fx = win.x + 10; let fy = win.y + 34;
        let fw = win.width - 20; let fh = win.height - 85; // 下部に編集用領域を確保
        if fw > 10 && fh > 10 {
            for py in fy..(fy + fh) { for px in fx..(fx + fw) { set_pixel(vram, px, py, 255, 255, 255); } }
            let mut item_y = fy + 6;
            for (idx, file) in kernel.files.iter().enumerate() {
                if item_y + 20 > fy + fh { break; }
                if Some(idx) == kernel.selected_file_idx {
                    for py in item_y..(item_y + 18) { for px in (fx + 4)..(fx + fw - 4) { set_pixel(vram, px, py, 0x00, 0x00, 0x80); } }
                }
                let icon = if file.is_image { "[I]" } else { "[T]" };
                let t_color = if Some(idx) == kernel.selected_file_idx { (255, 255, 255) } else { (0, 0, 0) };
                draw_string_scaled(vram, fx + 10, item_y + 2, icon, 0xff, 0xaa, 0x00);
                draw_string_scaled(vram, fx + 40, item_y + 2, &file.name, t_color.0, t_color.1, t_color.2);
                item_y += 22;
            }

            // 下部のメモ帳テキスト編集エディタ
            let ex = fx; let ey = fy + fh + 5; let ew = fw; let eh = 40;
            for py in ey..(ey + eh) { for px in ex..(ex + ew) { set_pixel(vram, px, py, 0xf5, 0xf5, 0xf5); } }
            
            // ボタンの描画
            let btn_x = ex + ew - 65; let btn_y = ey + 10;
            let btn_color = if kernel.is_editing_file { 0xff } else { 0xaa };
            for py in btn_y..(btn_y + 20) { for px in btn_x..(btn_x + 55) { set_pixel(vram, px, py, 0x44, btn_color, 0x44); } }
            draw_string_scaled(vram, btn_x + 8, btn_y + 4, "EDIT", 255, 255, 255);

            // 現在選択されているファイルの中身を表示（または編集用バッファ）
            if let Some(idx) = kernel.selected_file_idx {
                let display_text = if kernel.is_editing_file {
                    format!("{}_", kernel.cmd_buffer) // 入力中の文字
                } else {
                    kernel.files[idx].content.clone()
                };
                draw_string_scaled(vram, ex + 10, ey + 14, &display_text, 0, 0, 0);
            }
        }
    } else if win.id == 4 {
        // 👑 4. Task Manager（システム状況モニター）
        let mx = win.x + 10; let my = win.y + 34; let mw = win.width - 20; let mh = win.height - 45;
        if mw > 10 && mh > 10 {
            for py in my..(my + mh) { for px in mx..(mx + mw) { set_pixel(vram, px, py, 0x10, 0x20, 0x10); } }
            draw_string_scaled(vram, mx + 15, my + 15, "SYSTEM PERFORMANCE", 0x33, 0xff, 0x33);
            
            let cpu_line = format!("CPU USAGE : {}%", kernel.cpu_usage_dummy);
            draw_string_scaled(vram, mx + 15, my + 40, &cpu_line, 255, 255, 255);
            
            // パルスグラフの描画
            let gx = mx + 15; let gy = my + 70; let gw = mw - 30; let gh = mh - 90;
            if gw > 20 && gh > 20 {
                for px in gx..(gx + gw) {
                    set_pixel(vram, px, gy + gh - 1, 0, 0x88, 0); // ベースライン
                }
                // ダミーの波形を描画
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

fn draw_desktop_ui_high_quality(vram: &mut [u8]) {
    for px in 0..1280 { set_pixel(vram, px as i32, 688, 255, 255, 255); set_pixel(vram, px as i32, 689, 0x80, 0x80, 0x80); }
    for py in 690..720 { for px in 0..1280 { set_pixel(vram, px as i32, py, 0xd4, 0xd0, 0xc8); } }
    
    // タスクバーの4つのボタン
    let labels = [("TERM", 15, 85), ("DRAW", 100, 170), ("FILE", 185, 255), ("TASK", 270, 340)];
    let colors = [(0x00, 0x00, 0x80), (0xff, 0xaa, 0x00), (0x00, 0xaa, 0x55), (0x55, 0x55, 0x55)];
    for i in 0..4 {
        for py in 694..716 { for px in labels[i].1..labels[i].2 { set_pixel(vram, px, py, colors[i].0, colors[i].1, colors[i].2); } }
        draw_string_scaled(vram, labels[i].1 + 10, 700, labels[i].0, 255, 255, 255);
    }
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

#[wasm_bindgen]
pub fn on_os_keypress(key: &str) {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        
        // 👑 ファイル書き換え中の場合は、入力がメモ帳のバッファ（cmd_buffer）へ流れる
        if k.is_editing_file {
            if key == "Enter" {
                if let Some(idx) = k.selected_file_idx {
                    k.files[idx].content = k.cmd_buffer.clone(); // ファイル内容を上書き保存！
                    k.terminal_logs.push(format!("SAVED TO {}", k.files[idx].name));
                }
                k.cmd_buffer.clear();
                k.is_editing_file = false;
            } else if key == "Backspace" {
                k.cmd_buffer.pop();
            } else if key.len() == 1 {
                let c = key.chars().next().unwrap();
                if c.is_ascii_alphanumeric() || c == ' ' || c == '.' {
                    if k.cmd_buffer.len() < 30 { k.cmd_buffer.push(c.to_ascii_uppercase()); }
                }
            }
            return;
        }

        if !k.windows[0].is_open { return; }
        if key == "Enter" {
            let cmd = k.cmd_buffer.trim().to_lowercase();
            k.terminal_logs.push(format!(">{}", cmd));
            
            if cmd.starts_with("cat ") {
                let fname = cmd[4..].to_uppercase();
                let found = k.files.iter().find(|f| f.name == fname).map(|f| f.content.clone());
                match found {
                    Some(content) => k.terminal_logs.push(content),
                    None => k.terminal_logs.push("FILE NOT FOUND".to_string()),
                }
            } else {
                match cmd.as_str() {
                    "clear" => k.terminal_logs.clear(),
                    "help" => {
                        k.terminal_logs.push("CLEAR : CLEAR SCREEN".to_string());
                        k.terminal_logs.push("PAINT : RESET CANVAS".to_string());
                        k.terminal_logs.push("CAT [FILE] : READ FILE".to_string());
                    },
                    "paint" => {
                        k.paint_pixels = vec![255; 400 * 400]; k.terminal_logs.push("CANVAS RESET".to_string());
                    },
                    _ => k.terminal_logs.push("COMMAND NOT FOUND".to_string()),
                }
            }
            k.cmd_buffer.clear();
        } else if key == "Backspace" {
            k.cmd_buffer.pop();
        } else if key.len() == 1 {
            let c = key.chars().next().unwrap();
            if c.is_ascii_alphanumeric() || c == ' ' || c == '.' {
                if k.cmd_buffer.len() < 40 { k.cmd_buffer.push(c); }
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

        if my >= 694 && my <= 716 {
            if mx >= 15 && mx <= 85 { k.windows[0].is_open = true; }
            if mx >= 100 && mx <= 170 { k.windows[1].is_open = true; }
            if mx >= 185 && mx <= 255 { k.windows[2].is_open = true; }
            if mx >= 270 && mx <= 340 { k.windows[3].is_open = true; } // TASK
        }

        let mut clicked_close_id: Option<u8> = None;
        let mut clicked_resize_id: Option<u8> = None;
        let mut clicked_drag_id: Option<u8> = None;
        let mut drag_offset: (i32, i32) = (0, 0);
        let mut paint_pixel_idx: Option<usize> = None;
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
                if cx >= 0 && cx < k.paint_width && cy >= 0 && cy < k.paint_height { paint_pixel_idx = Some((cy * 400 + cx) as usize); break; }
            }
            if win.id == 3 {
                let fx = mx - (win.x + 10); let fy = my - (win.y + 34);
                let fw = win.width - 20; let fh = win.height - 85;
                
                // 👑 EDITボタンのクリック判定
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
        if let Some(idx) = paint_pixel_idx { k.paint_pixels[idx] = 0; }
        
        if file_click_idx.is_some() { k.is_editing_file = false; k.cmd_buffer.clear(); }

        if let Some(idx) = file_click_idx {
            k.selected_file_idx = Some(idx);
            let file_name = k.files[idx].name.clone();
            k.terminal_logs.push(format!("SELECTED: {}", file_name));
            if k.files[idx].is_image {
                for y in 30..90 { for x in 30..90 { k.paint_pixels[y * 400 + x] = 0; } }
            }
        }

        // 👑 EDITボタンを押した時、入力バッファに現在のファイルテキストをロード
        if edit_btn_clicked {
            if let Some(idx) = k.selected_file_idx {
                if !k.files[idx].is_image {
                    k.is_editing_file = !k.is_editing_file;
                    if k.is_editing_file {
                        k.cmd_buffer = k.files[idx].content.clone();
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
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) { win.width = (mx - ox).max(200); win.height = (my - oy).max(150); }
        }
        if let Some(id) = k.active_drag_id {
            let ox = k.drag_offset_x; let oy = k.drag_offset_y;
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) { win.x = mx - ox; win.y = my - oy; }
        }
        if k.is_mouse_down && k.active_drag_id.is_none() && k.active_resize_id.is_none() {
            if let Some(win) = k.windows.iter().find(|w| w.id == 2 && w.is_open) {
                let cx = mx - (win.x + 10); let cy = my - (win.y + 34);
                if cx >= 0 && cx < k.paint_width && cy >= 0 && cy < k.paint_height { k.paint_pixels[(cy * 400 + cx) as usize] = 0; }
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
