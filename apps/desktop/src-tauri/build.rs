use image::imageops::FilterType;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../appIcon.png");
    println!("cargo:rerun-if-changed=../../appIcon.ico");
    println!("cargo:rerun-if-changed=../../appIcon.icns");

    if let Err(err) = sync_app_icon_assets() {
        println!("cargo:warning=failed to sync app icon assets: {err}");
    }

    tauri_build::build()
}

fn sync_app_icon_assets() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir.join("../../..");
    let icons_dir = manifest_dir.join("icons");

    let app_icon_png = repo_root.join("appIcon.png");
    let app_icon_ico = repo_root.join("appIcon.ico");
    let app_icon_icns = repo_root.join("appIcon.icns");

    if !app_icon_png.exists() && !app_icon_ico.exists() && !app_icon_icns.exists() {
        println!(
            "cargo:warning=appIcon not found in repo root (expected appIcon.png/ico/icns); keeping existing icons"
        );
        return Ok(());
    }

    std::fs::create_dir_all(&icons_dir)?;

    if app_icon_png.exists() {
        let img = image::open(&app_icon_png)?;
        write_resized_png(&img, &icons_dir.join("32x32.png"), 32, 32)?;
        write_resized_png(&img, &icons_dir.join("128x128.png"), 128, 128)?;
        write_resized_png(&img, &icons_dir.join("128x128@2x.png"), 256, 256)?;
        write_resized_png(&img, &icons_dir.join("icon.png"), 512, 512)?;

        if !app_icon_ico.exists() {
            // Best-effort: generate Windows icon when only PNG is provided.
            let ico_img = img.resize_exact(256, 256, FilterType::Lanczos3);
            ico_img.save_with_format(icons_dir.join("icon.ico"), image::ImageFormat::Ico)?;
        }
    }

    if app_icon_ico.exists() {
        std::fs::copy(&app_icon_ico, icons_dir.join("icon.ico"))?;
    }

    if app_icon_icns.exists() {
        std::fs::copy(&app_icon_icns, icons_dir.join("icon.icns"))?;
    } else if app_icon_png.exists() {
        generate_icns_from_png(&image::open(&app_icon_png)?, &manifest_dir, &icons_dir)?;
    } else {
        println!(
            "cargo:warning=appIcon.icns not found; macOS bundle will use existing icons/icon.icns"
        );
    }

    Ok(())
}

fn write_resized_png(
    img: &image::DynamicImage,
    out_path: &Path,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let resized = img.resize_exact(width, height, FilterType::Lanczos3);
    resized.save_with_format(out_path, image::ImageFormat::Png)?;
    Ok(())
}

fn generate_icns_from_png(
    img: &image::DynamicImage,
    manifest_dir: &Path,
    icons_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        let iconset_dir = manifest_dir.join("icons").join(".appicon.iconset");
        if iconset_dir.exists() {
            std::fs::remove_dir_all(&iconset_dir)?;
        }
        std::fs::create_dir_all(&iconset_dir)?;

        let specs = [
            (16u32, "icon_16x16.png"),
            (32u32, "icon_16x16@2x.png"),
            (32u32, "icon_32x32.png"),
            (64u32, "icon_32x32@2x.png"),
            (128u32, "icon_128x128.png"),
            (256u32, "icon_128x128@2x.png"),
            (256u32, "icon_256x256.png"),
            (512u32, "icon_256x256@2x.png"),
            (512u32, "icon_512x512.png"),
            (1024u32, "icon_512x512@2x.png"),
        ];
        for (size, name) in specs {
            write_resized_png(img, &iconset_dir.join(name), size, size)?;
        }

        let out_icns = icons_dir.join("icon.icns");
        let output = Command::new("iconutil")
            .arg("-c")
            .arg("icns")
            .arg(&iconset_dir)
            .arg("-o")
            .arg(&out_icns)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!(
                "cargo:warning=failed to generate icon.icns via iconutil: {}",
                stderr.trim()
            );
        }
        let _ = std::fs::remove_dir_all(&iconset_dir);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (img, manifest_dir, icons_dir);
        println!(
            "cargo:warning=appIcon.icns missing and current build host is not macOS; keeping existing icon.icns"
        );
    }

    Ok(())
}
