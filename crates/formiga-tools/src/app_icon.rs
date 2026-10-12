//! The app's icons, made from the mascot drawing: the Mac's .icns, the Windows .ico, a large PNG
//! and the tray icon.

use crate::write_png;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

/// The size of the icon the app puts in the menu bar or the notification area, in pixels.
const TRAY_ICON_SIZE: u32 = 64;

pub fn app_icon(directory: PathBuf, source: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&directory)
        .with_context(|| format!("create {}", directory.display()))?;
    let png_path = directory.join("Formiga.png");
    let ico_path = directory.join("Formiga.ico");
    let icns_path = directory.join("Formiga.icns");
    let tray_path = directory.join("Formiga-tray.png");

    let (source_width, source_height, source_pixels) = read_rgba_png(&source)?;
    anyhow::ensure!(
        source_width == source_height,
        "icon source must be square, got {source_width}x{source_height}"
    );
    let mac_pixels = resize_rgba_square(&source_pixels, source_width, 1024);
    write_png(&png_path, 1024, 1024, &mac_pixels)?;

    let icon_sizes = [
        (*b"ic10", 1024_u32),
        (*b"ic09", 512),
        (*b"ic08", 256),
        (*b"ic07", 128),
        (*b"icp5", 32),
        (*b"icp4", 16),
    ];
    let mut icns_chunks = Vec::new();
    for (kind, size) in icon_sizes {
        let pixels = resize_rgba_square(&source_pixels, source_width, size);
        icns_chunks.push((kind, encode_png(size, size, &pixels)?));
    }
    let icns_length = 8_usize
        + icns_chunks
            .iter()
            .map(|(_, png)| 8 + png.len())
            .sum::<usize>();
    let mut icns = BufWriter::new(
        File::create(&icns_path).with_context(|| format!("create {}", icns_path.display()))?,
    );
    icns.write_all(b"icns")?;
    icns.write_all(&(icns_length as u32).to_be_bytes())?;
    for (kind, png) in icns_chunks {
        icns.write_all(&kind)?;
        icns.write_all(&((png.len() + 8) as u32).to_be_bytes())?;
        icns.write_all(&png)?;
    }
    icns.flush()?;

    // Modern Windows icon resources can contain a PNG-compressed 256 px image. Writing the tiny
    // ICO container here keeps packaging deterministic and avoids an image-conversion dependency.
    let windows_pixels = resize_rgba_square(&source_pixels, source_width, 256);
    let png_bytes = encode_png(256, 256, &windows_pixels)?;
    let mut ico = BufWriter::new(
        File::create(&ico_path).with_context(|| format!("create {}", ico_path.display()))?,
    );
    ico.write_all(&0_u16.to_le_bytes())?; // reserved
    ico.write_all(&1_u16.to_le_bytes())?; // image
    ico.write_all(&1_u16.to_le_bytes())?; // one entry
    ico.write_all(&[0, 0, 0, 0])?; // 256x256, true color, reserved
    ico.write_all(&1_u16.to_le_bytes())?; // color planes
    ico.write_all(&32_u16.to_le_bytes())?;
    ico.write_all(&(png_bytes.len() as u32).to_le_bytes())?;
    ico.write_all(&22_u32.to_le_bytes())?;
    ico.write_all(&png_bytes)?;
    ico.flush()?;

    // The menu bar on a Mac and the notification area on Windows show the same picture, small:
    // the app embeds this and lets each system fit it to its own bar.
    let tray_pixels = resize_rgba_square(&source_pixels, source_width, TRAY_ICON_SIZE);
    write_png(&tray_path, TRAY_ICON_SIZE, TRAY_ICON_SIZE, &tray_pixels)?;

    println!(
        "wrote {}, {}, {}, and {}",
        png_path.display(),
        icns_path.display(),
        ico_path.display(),
        tray_path.display()
    );
    Ok(())
}

fn encode_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    formiga_art::write_png(&mut bytes, width, height, pixels)?;
    Ok(bytes)
}

fn read_rgba_png(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let size = reader
        .output_buffer_size()
        .context("decoded icon is too large")?;
    let mut pixels = vec![0; size];
    let info = reader.next_frame(&mut pixels)?;
    pixels.truncate(info.buffer_size());
    anyhow::ensure!(
        info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight,
        "icon source must decode to 8-bit RGBA"
    );
    Ok((info.width, info.height, pixels))
}

fn resize_rgba_square(source: &[u8], source_size: u32, target_size: u32) -> Vec<u8> {
    let mut output = vec![0; (target_size * target_size * 4) as usize];
    for y in 0..target_size {
        let source_y = y * source_size / target_size;
        for x in 0..target_size {
            let source_x = x * source_size / target_size;
            let source_index = ((source_y * source_size + source_x) * 4) as usize;
            let target_index = ((y * target_size + x) * 4) as usize;
            output[target_index..target_index + 4]
                .copy_from_slice(&source[source_index..source_index + 4]);
        }
    }
    output
}
