#!/usr/bin/env nu

def main [tape: path, output_directory: path] {
    let scene = $tape | path parse | get stem
    let video_name = $"($scene).mp4"
    let webp_name = $"($scene).webp"
    ^vhs $tape
    (
        ^ffmpeg
        -loglevel error
        -i $video_name
        -vf mpdecimate
        -fps_mode vfr
        -c:v libwebp_anim
        -lossless 0
        -q:v 80
        -compression_level 6
        -loop 0
        $webp_name
    )
    mkdir $output_directory
    cp $webp_name ($output_directory | path join $webp_name)
}
