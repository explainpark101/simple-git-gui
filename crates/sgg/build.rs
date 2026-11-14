use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let png_path = manifest_dir.join("assets/icon/app-icon-1024.png");
    println!("cargo:rerun-if-changed={}", png_path.display());

    // Prefer CARGO_CFG_TARGET_OS: build.rs always runs on the host.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Err(err) = embed_windows_icon(&png_path) {
            println!("cargo:warning=failed to embed Windows app icon: {err:#}");
        }
    }
}

fn embed_windows_icon(png_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use image::imageops::FilterType;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let ico_path = out_dir.join("app-icon.ico");

    let src = image::open(png_path)?;
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16_u32, 32, 48, 64, 128, 256] {
        let resized = src.resize_exact(size, size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
        icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image)?);
    }

    let file = fs::File::create(&ico_path)?;
    icon_dir.write(file)?;

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().ok_or("icon path is not valid UTF-8")?);
    res.set("ProductName", "simple-git-gui");
    res.set("FileDescription", "simple-git-gui (sgg)");
    res.compile()?;
    Ok(())
}