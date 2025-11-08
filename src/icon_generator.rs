// Simple icon generator - creates a 32x32 icon with a document/PDF symbol
use image::{ImageBuffer, Rgba, RgbaImage};

pub fn generate_icon_data() -> Vec<u8> {
    let width = 32;
    let height = 32;

    let mut img: RgbaImage = ImageBuffer::new(width, height);

    // Background color - transparent
    for pixel in img.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 0]);
    }

    // Draw a document icon (simplified)
    // Document body - white rectangle with blue border
    for y in 4..28 {
        for x in 8..24 {
            if x == 8 || x == 23 || y == 4 || y == 27 {
                // Border - blue
                img.put_pixel(x, y, Rgba([41, 128, 185, 255]));
            } else {
                // Fill - white
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
    }

    // Draw folded corner (top right)
    for y in 4..9 {
        for x in 19..24 {
            if x + y < 27 {
                img.put_pixel(x, y, Rgba([189, 195, 199, 255]));
            }
        }
    }

    // Draw lines to represent text
    for y in [12, 16, 20, 24] {
        for x in 11..21 {
            img.put_pixel(x, y, Rgba([52, 73, 94, 255]));
        }
    }

    // Encode to PNG
    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("Failed to encode image");

    bytes
}

#[allow(dead_code)]
pub fn save_icon_to_file(path: &str) -> std::io::Result<()> {
    let bytes = generate_icon_data();
    std::fs::write(path, bytes)?;
    Ok(())
}
