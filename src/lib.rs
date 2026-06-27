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

// 👑 ファイルを表現する構造体
struct VirtualFile {
    name: String,
    content: String,
    is_image: bool, // ターミナル用テキストか、ペイント用データか
}

struct OSKernel {
    vram: Vec<u8>,
    windows: Vec<WindowState>,
    files: Vec<VirtualFile>,      // 👑 擬似ファイルシステム
    selected_file_idx: Option<usize>, // 現在選択されているファイル
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
}

thread_local! {
    static KERNEL: RefCell<OSKernel> = RefCell::new(OSKernel {
        vram: vec![0; VRAM_SIZE],
        windows: vec![
            WindowState { id: 1, title: "WasmOS Terminal v1.2".to_string(), x: 60, y: 80, width: 420, height: 300, is_open: true },
            WindowState { id: 2, title: "VRAM Paint app Pro".to_string(), x: 510, y: 80, width: 420, height: 350, is_open: true },
            // 👑 3つ目のウィンドウ「File Manager」を追加！
            WindowState { id: 3, title: "File Manager".to_string(), x: 200, y: 410, width: 300, height: 240, is_open: true },
        ],
        // 👑 初期ファイルを用意
        files: vec![
            VirtualFile { name: "README.TXT".to_string(), content: "WELCOME TO WASMOS! THIS IS A TEXT FILE.".to_string(), is_image: false },
            VirtualFile { name: "SYSTEM.TXT".to_string(), content: "KERNEL V1.2 MEMORY SHARED VRAM ACTIVE.".to_string(), is_image: false },
            VirtualFile { name: "ICON.BMP".to_string(), content: "IMAGE_DATA".to_string(), is_image: true },
        ],
        selected_file_idx: None,
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
            "WASM OS [Version 1.2.720p]".to_string(),
            "File application engine loaded successfully.".to_string(),
            "Type 'cat [filename]' or click files in File Manager.".to_string()
        ],
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
                let c_i32 = col as i32;
                let r_i32 = row as i32;
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
    for c in text.chars() {
        draw_char_scaled(vram, cur_x, y, c, r, g, b);
        cur_x += 9;
    }
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
    for i in (0..VRAM_SIZE).step_by(4) {
        vram[i] = r; vram[i + 1] = g; vram[i + 2] = b; vram[i + 3] = 255;
    }
}

fn draw_window_high_quality(vram: &mut [u8], win: &WindowState, kernel: &OSKernel) {
    if !win.is_open { return; }

    // 背景グレー
    for py in win.y..(win.y + win.height) {
        for px in win.x..(win.x + win.width) { set_pixel(vram, px, py, 0xd4, 0xd0, 0xc8); }
    }
    // 立体外枠
    for px in win.x..(win.x + win.width) {
        set_pixel(vram, px, win.y, 255, 255, 255);
        set_pixel(vram, px, win.y + win.height - 1, 0x40, 0x40, 0x40);
    }
    for py in win.y..(win.y + win.height) {
        set_pixel(vram, win.x, py, 255, 255, 255);
        set_pixel(vram, win.x + win.width - 1, py, 0x40, 0x40, 0x40);
    }
    // タイトルバーグラデーション
    for py in (win.y + 3)..(win.y + 28) {
        let t = (py - (win.y + 3)) as f32 / 25.0;
        let r = (0x00 as f32 * (1.0 - t) + 0x0a as f32 * t) as u8;
        let g = (0x00 as f32 * (1.0 - t) + 0x5a as f32 * t) as u8;
        let b = (0x80 as f32 * (1.0 - t) + 0xff as f32 * t) as u8;
        for px in (win.x + 4)..(win.x + win.width - 4) { set_pixel(vram, px, py, r, g, b); }
    }
    draw_string_scaled(vram, win.x + 12, win.y + 8, &win.title, 255, 255, 255);

    // ✕ ボタン
    let bx = win.x + win.width - 24; let by = win.y + 6;
    for py in by..(by + 16) {
        for px in bx..(bx + 16) { set_pixel(vram, px, py, 0xe0, 0x40, 0x40); }
    }
    draw_string_scaled(vram, bx + 5, by + 3, "X", 255, 255, 255);

    // サイズ変更つまみ
    let rx = win.x + win.width - 12; let ry = win.y + win.height - 12;
    for i in 0..10 {
        for j in i..10 { if (i + j) % 3 == 0 { set_pixel(vram, rx + i, ry + j, 0x80, 0x80, 0x80); } }
    }

    // アプリ個別領域のレンダリング
    if win.id == 1 {
        // 1. ターミナル
        let tx = win.x + 8; let ty = win.y + 34;
        let tw = win.width - 16; let th = win.height - 42;
        if tw > 20 && th > 40 {
            for py in ty..(ty + th) {
                for px in tx..(tx + tw) { set_pixel(vram, px, py, 0x05, 0x05, 0x05); }
            }
            let mut line_y = ty + 10;
            for log in &kernel.terminal_logs {
                if line_y + 14 < ty + th - 25 {
                    draw_string_scaled(vram, tx + 12, line_y, log, 0x33, 0xff, 0x33);
                    line_y += 16;
                }
            }
            let prompt = format!(">{}", kernel.cmd_buffer);
            if ty + th - 20 > ty { draw_string_scaled(vram, tx + 12, ty + th - 20, &prompt, 255, 255, 255); }
        }
    } else if win.id == 2 {
        // 2. ペイント
        let cx = win.x + 10; let cy = win.y + 34;
        let cw = win.width - 20; let ch = win.height - 45;
        if cw > 10 && ch > 10 {
            for py in cy..(cy + ch) {
                for px in cx..(cx + cw) {
                    let dx = px - cx; let dy = py - cy;
                    if dx >= 0 && dx < kernel.paint_width && dy >= 0 && dy < kernel.paint_height {
                        let p_color = kernel.paint_pixels[(dy * 400 + dx) as usize];
                        set_pixel(vram, px, py, p_color, p_color, p_color);
                    } else { set_pixel(vram, px, py, 0xaa, 0xaa, 0xaa); }
                }
            }
        }
    } else if win.id == 3 {
        // 👑 3. File Manager（ファイルエクスプローラー描画）
        let fx = win.x + 10; let fy = win.y + 34;
        let fw = win.width - 20; let fh = win.height - 45;
        if fw > 10 && fh > 10 {
            // 内側のホワイト背景
            for py in fy..(fy + fh) {
                for px in fx..(fx + fw) { set_pixel(vram, px, py, 255, 255, 255); }
            }
            // 各ファイルを行としてレンダリング
            let mut item_y = fy + 8;
            for (idx, file) in kernel.files.iter().enumerate() {
                if item_y + 24 > fy + fh { break; }
                
                // 選択されているファイルは青くハイライト
                let is_selected = Some(idx) == kernel.selected_file_idx;
                if is_selected {
                    for py in item_y..(item_y + 20) {
                        for px in (fx + 4)..(fx + fw - 4) { set_pixel(vram, px, py, 0x00, 0x00, 0x80); }
                    }
                }

                // アイコンの代わりのマーク [T]:テキスト [I]:画像
                let icon = if file.is_image { "[I]" } else { "[T]" };
                let txt_color = if is_selected { (255, 255, 255) } else { (0, 0, 0) };
                
                draw_string_scaled(vram, fx + 10, item_y + 3, icon, 0xff, 0xaa, 0x00);
                draw_string_scaled(vram, fx + 40, item_y + 3, &file.name, txt_color.0, txt_color.1, txt_color.2);
                
                item_y += 24;
            }
        }
    }
}

fn draw_desktop_ui_high_quality(vram: &mut [u8]) {
    for px in 0..1280 {
        set_pixel(vram, px as i32, 688, 255, 255, 255);
        set_pixel(vram, px as i32, 689, 0x80, 0x80, 0x80);
    }
    for py in 690..720 {
        for px in 0..1280 { set_pixel(vram, px as i32, py, 0xd4, 0xd0, 0xc8); }
    }
    
    // タスクバーの3つのボタン
    let labels = [("TERM", 15, 85), ("DRAW", 100, 170), ("FILE", 185, 255)];
    let colors = [(0x00, 0x00, 0x80), (0xff, 0xaa, 0x00), (0x00, 0xaa, 0x55)];
    
    for i in 0..3 {
        for py in 694..716 {
            for px in labels[i].1..labels[i].2 { set_pixel(vram, px, py, colors[i].0, colors[i].1, colors[i].2); }
        }
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
        if !k.windows[0].is_open { return; }
        
        if key == "Enter" {
            let cmd = k.cmd_buffer.trim().to_lowercase();
            k.terminal_logs.push(format!(">{}", cmd));
            
            if cmd.starts_with("cat ") {
                let fname = cmd.substring(4).to_uppercase();
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
                        k.paint_pixels = vec![255; 400 * 400];
                        k.terminal_logs.push("CANVAS BUFFER RESET".to_string());
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
        }

        let mut clicked_close_id: Option<u8> = None;
        let mut clicked_resize_id: Option<u8> = None;
        let mut clicked_drag_id: Option<u8> = None;
        let mut drag_offset: (i32, i32) = (0, 0);
        let mut paint_pixel_idx: Option<usize> = None;
        let mut file_click_idx: Option<usize> = None;
        
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
                    paint_pixel_idx = Some((cy * 400 + cx) as usize); break;
                }
            }
            
            if win.id == 3 {
                let fx = mx - (win.x + 10); let fy = my - (win.y + 34);
                let fw = win.width - 20; let fh = win.height - 45;
                if fx >= 0 && fx < fw && fy >= 0 && fy < fh {
                    let clicked_row = (fy - 8) / 24;
                    if clicked_row >= 0 && (clicked_row as usize) < k.files.len() {
                        file_click_idx = Some(clicked_row as usize);
                        break;
                    }
                }
            }
        }

        if let Some(id) = clicked_close_id {
            if let Some(w) = k.windows.iter_mut().find(|w| w.id == id) { w.is_open = false; }
        }
        if let Some(id) = clicked_resize_id {
            k.active_resize_id = Some(id); k.drag_offset_x = drag_offset.0; k.drag_offset_y = drag_offset.1;
        }
        if let Some(id) = clicked_drag_id {
            k.active_drag_id = Some(id); k.drag_offset_x = drag_offset.0; k.drag_offset_y = drag_offset.1;
        }
        if let Some(idx) = paint_pixel_idx { k.paint_pixels[idx] = 0; }
        
        if let Some(idx) = file_click_idx {
            k.selected_file_idx = Some(idx);
            let file_name = k.files[idx].name.clone();
            let file_content = k.files[idx].content.clone();
            
            k.terminal_logs.push(format!("OPENING FILE: {}", file_name));
            if !k.files[idx].is_image {
                k.terminal_logs.push(file_content);
            } else {
                k.terminal_logs.push("BMP DRAWN TO CANVAS".to_string());
                for y in 40..80 {
                    for x in 40..80 { k.paint_pixels[y * 400 + x] = 0; }
                }
            }
        }
    });
    render_all();
}

// 👑 抜け落ちていたマウス移動とマウスアップの処理を追加！
#[wasm_bindgen]
pub fn on_mouse_move(mx: f64, my: f64) {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        let mx = mx as i32; let my = my as i32;
        k.mouse_x = mx; k.mouse_y = my;
        
        if let Some(id) = k.active_resize_id {
            let ox = k.drag_offset_x; let oy = k.drag_offset_y;
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) {
                win.width = (mx - ox).max(180);  
                win.height = (my - oy).max(120); 
            }
        }
        if let Some(id) = k.active_drag_id {
            let ox = k.drag_offset_x; let oy = k.drag_offset_y;
            if let Some(win) = k.windows.iter_mut().find(|w| w.id == id) { win.x = mx - ox; win.y = my - oy; }
        }
        if k.is_mouse_down && k.active_drag_id.is_none() && k.active_resize_id.is_none() {
            if let Some(win) = k.windows.iter().find(|w| w.id == 2 && w.is_open) {
                let cx = mx - (win.x + 10); let cy = my - (win.y + 34);
                if cx >= 0 && cx < k.paint_width && cy >= 0 && cy < k.paint_height {
                    let idx = (cy * 400 + cx) as usize; k.paint_pixels[idx] = 0;
                }
            }
        }
    });
    render_all();
}

#[wasm_bindgen]
pub fn on_mouse_up() {
    KERNEL.with(|k| {
        let mut k = k.borrow_mut();
        k.active_drag_id = None; k.active_resize_id = None; k.is_mouse_down = false;
    });
    render_all();
}

// 簡易的な文字列切り出しの補助トレイト（すでに末尾にある場合は重複に注意してください）
trait Substring { fn substring(&self, start: usize) -> &str; }
impl Substring for str {
    fn substring(&self, start: usize) -> &str {
        if start >= self.len() { "" } else { &self[start..] }
    }
}
