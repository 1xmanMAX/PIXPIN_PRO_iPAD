//! Resampling helpers on interleaved 8-bit buffers.

use image::imageops::FilterType;
use image::{ImageBuffer, Luma, Rgb, Rgba};

pub fn resize(
    data: &[u8],
    w: u32,
    h: u32,
    channels: u8,
    nw: u32,
    nh: u32,
    filter: FilterType,
) -> Vec<u8> {
    let (nw, nh) = (nw.max(1), nh.max(1));
    match channels {
        1 => {
            let img: ImageBuffer<Luma<u8>, _> =
                ImageBuffer::from_raw(w, h, data.to_vec()).expect("size");
            image::imageops::resize(&img, nw, nh, filter).into_raw()
        }
        3 => {
            let img: ImageBuffer<Rgb<u8>, _> =
                ImageBuffer::from_raw(w, h, data.to_vec()).expect("size");
            image::imageops::resize(&img, nw, nh, filter).into_raw()
        }
        4 => {
            // CMYK resampled channel-wise; Rgba is just four independent planes here.
            let img: ImageBuffer<Rgba<u8>, _> =
                ImageBuffer::from_raw(w, h, data.to_vec()).expect("size");
            image::imageops::resize(&img, nw, nh, filter).into_raw()
        }
        _ => data.to_vec(),
    }
}

pub fn downscale(data: &[u8], w: u32, h: u32, channels: u8, nw: u32, nh: u32) -> Vec<u8> {
    resize(data, w, h, channels, nw, nh, FilterType::Lanczos3)
}

pub fn upscale(data: &[u8], w: u32, h: u32, channels: u8, nw: u32, nh: u32) -> Vec<u8> {
    resize(data, w, h, channels, nw, nh, FilterType::Triangle)
}
