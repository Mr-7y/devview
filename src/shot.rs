//! 仅 debug 编译：按 F12 截取窗口并写成 PNG，用于和设计稿逐像素对比。

use egui::ColorImage;
use std::path::{Path, PathBuf};

/// F12 时向渲染器请求一帧截图。
pub fn want(ctx: &egui::Context) {
    if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
    }
}

/// 渲染器回传截图后落盘，返回路径与尺寸。
pub fn take(ctx: &egui::Context) -> Option<(PathBuf, (usize, usize))> {
    let img = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    })?;
    let path = std::env::temp_dir().join("devview-shot.png");
    save_png(&path, &img).ok()?;
    Some((path, (img.size[0], img.size[1])))
}

fn save_png(path: &Path, img: &ColorImage) -> std::io::Result<()> {
    // 2 倍降采样：截图只用于观感对比，不需要原始分辨率
    let (sw, sh) = (img.size[0], img.size[1]);
    let (w, h) = (sw / 2, sh / 2);
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            let c = img.pixels[(y * 2) * sw + x * 2];
            raw.extend_from_slice(&[c.r(), c.g(), c.b()]);
        }
    }

    let mut png: Vec<u8> = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &ihdr);

    // zlib 容器 + deflate 的 stored 块：不做压缩，换取零依赖
    let mut z = vec![0x78, 0x01];
    let mut i = 0;
    while i < raw.len() {
        let n = (raw.len() - i).min(65535);
        let last = i + n == raw.len();
        z.push(if last { 1 } else { 0 });
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&raw[i..i + n]);
        i += n;
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    write_chunk(&mut png, b"IDAT", &z);
    write_chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, png)
}

fn write_chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    let mut crc = 0xffff_ffffu32;
    for b in ty.iter().chain(data.iter()) {
        crc = crc_byte(crc, *b);
    }
    out.extend_from_slice(&(crc ^ 0xffff_ffff).to_be_bytes());
}

fn crc_byte(mut crc: u32, b: u8) -> u32 {
    crc ^= b as u32;
    for _ in 0..8 {
        crc = if crc & 1 != 0 {
            (crc >> 1) ^ 0xedb8_8320
        } else {
            crc >> 1
        };
    }
    crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for x in data {
        a = (a + *x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
