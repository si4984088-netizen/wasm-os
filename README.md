# WasmOS

ブラウザで動く小さなデスクトップ OS です。Rust で書いたカーネルを WebAssembly にコンパイルし、1920×1080 のフレームバッファ（VRAM）をピクセル単位で描画しています。DOM で画面を作るのではなく、OS 側が全ピクセルを描いて Canvas に転送する方式です。

**▶ デモ: https://si4984088-netizen.github.io/wasm-os/**

## 画面とアプリ

- **Terminal** — コマンドライン（下記）。起動時に開いています
- **VRAM Paint** — お絵描き。グレースケールのパレットで筆の濃さを変えられます
- **File Manager & Notepad** — ファイル一覧とテキスト編集。ファイルはブラウザの localStorage に保存され、再読み込みしても残ります
- **Task Manager** — 稼働時間・ウィンドウ数・ファイル数などの実状態を表示

ウィンドウはドラッグで移動、右下の隅でリサイズ、右上の × で閉じられます。下のタスクバーからいつでも開き直せます。

## ターミナル命令

| 命令 | 説明 |
|---|---|
| `help` | 命令の一覧 |
| `clear` / `cls` | 画面を消す |
| `ver` | バージョン表示 |
| `echo [text]` | 文字を表示 |
| `ls` / `dir` | ファイル一覧 |
| `cat [file]` | ファイルを読む |
| `touch [name]` | 空のファイルを作成 |
| `edit [file]` | メモ帳で編集（Enter で保存、Esc で取消） |
| `rm [file]` | ファイルを削除 |
| `mv [old] [new]` | 名前を変更 |
| `calc [式]` | 計算（例: `calc 12*(3+4)` → `= 84`） |
| `time` / `date` | 現在時刻 |
| `open [app]` / `close [app]` | アプリを開く/閉じる（term / paint / file / task） |
| `ps` | ウィンドウ一覧 |
| `mem` | システム情報 |
| `paint` | キャンバスをリセット |
| `brush [0-255]` | ペイントの筆の濃さ |
| `about` | この OS について |
| `exit` | ターミナルを閉じる |

## ビルド方法

必要なもの: Rust（stable）と wasm-bindgen CLI（Cargo.lock の wasm-bindgen と同じバージョン）。

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/wasm_os.wasm
```

`pkg/` が生成物です。`index.html` と `pkg/` を静的ホスティングに置けば動きます（wasm-pack を使う場合は `wasm-pack build --target web --out-dir pkg` でも同じ構成になります）。

## 構成

- `src/lib.rs` — カーネル本体（VRAM 描画、ウィンドウ管理、ターミナル、疑似ファイルシステム）
- `index.html` — Canvas への転送とマウス/キーボード入力の中継。表示が縮小されても座標がずれないよう補正しています
- `pkg/` — wasm-bindgen の生成物（コミット済み。GitHub Pages はこれを配信します）
