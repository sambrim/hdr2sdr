# hdr2sdr

A lightweight command-line tool written in Rust that checks if a video file is HDR and safely converts it to SDR using `ffprobe` and `ffmpeg`.

### TL;DR
Checks if a video file has a colorspace that mathes HDR, converts it to SDR BT709.

## What It Does

1. **Checks the video first:** Uses `ffprobe` to inspect the video's color metadata (BT.2020 colorspace/primaries, PQ, or HLG). If the video is already SDR, it skips it.
2. **Converts with proper tone mapping:** Uses `ffmpeg` (`zscale` and Hable tone mapping) to convert HDR colors to standard BT.709 SDR without leaving the picture looking washed out or grey.
3. **Safely replaces the original:** Writes the converted video to a temporary file first. Only after `ffmpeg` finishes successfully does it replace the original file—so your original video stays untouched if anything goes wrong.

## Prerequisites

* **FFmpeg & FFprobe:** Must be installed and available in your system `PATH`.
  ```bash
  apt install ffmpeg
  dnf install ffmpeg
  winget install ffmpeg
