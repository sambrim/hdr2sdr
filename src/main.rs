use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<VideoStream>,
}

#[derive(Debug, Deserialize)]
struct VideoStream {
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        bail!("Usage: {} <path/to/video>", args[0]);
    }

    let input_path = PathBuf::from(&args[1]);
    if !input_path.is_file() {
        bail!("Input file does not exist: {}", input_path.display());
    }

    println!("Inspecting {}...", input_path.display());
    if !is_hdr(&input_path)? {
        println!("Video is already SDR (not BT.2020/PQ/HLG). Skipping.");
        return Ok(());
    }

    println!("HDR stream detected. Converting to BT.709 SDR...");
    convert_and_replace(&input_path)?;
    println!("Conversion complete. Original file safely replaced.");

    Ok(())
}

/// Checks color_space, color_primaries, and color_transfer via ffprobe.
fn is_hdr(path: &Path) -> Result<bool> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-select_streams",
            "v:0",
        ])
        .arg(path)
        .output()
        .context("Failed to execute ffprobe. Is FFmpeg installed and in PATH?")?;

    if !output.status.success() {
        bail!(
            "ffprobe exited with error: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let parsed: FfprobeOutput =
        serde_json::from_slice(&output.stdout).context("Failed to parse ffprobe JSON output")?;

    let stream = parsed
        .streams
        .first()
        .context("No video stream found in file")?;

    let space = stream.color_space.as_deref().unwrap_or("");
    let primaries = stream.color_primaries.as_deref().unwrap_or("");
    let transfer = stream.color_transfer.as_deref().unwrap_or("");

    let is_bt2020 = space.starts_with("bt2020") || primaries == "bt2020";
    let is_hdr_transfer = matches!(transfer, "smpte2084" | "arib-std-b67");

    Ok(is_bt2020 || is_hdr_transfer)
}

/// Converts the video to a temporary file in the same directory, then replaces the original.
fn convert_and_replace(input_path: &Path) -> Result<()> {
    let temp_path = build_temp_path(input_path)?;

    // Tone-map BT.2020 (PQ/HLG) to BT.709 SDR using zscale + Hable tone mapping
    let vf_filter = "zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,\
                     tonemap=tonemap=hable:desat=0,\
                     zscale=t=bt709:m=bt709:r=tv,format=yuv420p";

    let status = Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(input_path)
        .args([
            "-vf",
            vf_filter,
            "-c:v",
            "libx264",
            "-crf",
            "18",
            "-preset",
            "medium",
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-c:a",
            "copy",
        ])
        .arg(&temp_path)
        .status()
        .context("Failed to execute ffmpeg")?;

    if !status.success() {
        // Clean up partial temp file if ffmpeg failed
        let _ = fs::remove_file(&temp_path);
        bail!("ffmpeg conversion failed; original file was not modified.");
    }

    // Verify temp file exists and is non-empty before overwriting original
    let metadata = fs::metadata(&temp_path).context("Temporary output file is missing")?;
    if metadata.len() == 0 {
        let _ = fs::remove_file(&temp_path);
        bail!("Temporary output file is empty; aborting replacement.");
    }

    // Atomically replace the original file (deletes original on Unix/Windows)
    fs::rename(&temp_path, input_path).context("Failed to replace original file with SDR output")?;

    Ok(())
}

/// Creates a temporary filename in the same folder (e.g., `movie.mp4` -> `movie.sdr_tmp.mp4`)
/// so `fs::rename` stays on the same filesystem.
fn build_temp_path(original: &Path) -> Result<PathBuf> {
    let parent = original.parent().unwrap_or_else(|| Path::new("."));
    let stem = original
        .file_stem()
        .context("Invalid input filename")?;
    let ext = original.extension().unwrap_or_default();

    let mut temp_name = OsString::from(stem);
    temp_name.push(".sdr_tmp");
    if !ext.is_empty() {
        temp_name.push(".");
        temp_name.push(ext);
    }

    Ok(parent.join(temp_name))
}
