use std::io::{Result, Read, Cursor, Write};
use std::num::NonZeroU64;
use std::time::Instant;

use flate2::read::GzDecoder;
use zopfli::Format::Gzip;


fn main() {
    let usage = "Usage: nbt-compress [-i <iterations>] file1 file2 ...";
    let args: Vec<String> = std::env::args().collect();
    let mut iterations = -1;
    let mut files = Vec::new();

    for (index, arg) in args.iter().enumerate() {
        if arg.starts_with("-i") {
            iterations = parse_arg(&arg, &args, index).unwrap_or_else(|e| {
                eprintln!("Error parsing iterations: {}", e);
                std::process::exit(1);
            });
        } else if arg.starts_with("--iterations") {
            iterations = parse_arg(&arg, &args, index).unwrap_or_else(|e| {
                eprintln!("Error parsing iterations: {}", e);
                std::process::exit(1);
            });
        } else if arg.eq("-h") || arg.eq("--help") {
            println!("{}", usage);
            std::process::exit(0);
        } else if index > 0 {
            // Skip the first argument (program name)
            files.push(arg.clone());
        }
    }

    if files.is_empty() {
        println!("{}", usage);
        std::process::exit(1);
    }

    for file in &files {
        compress_file(file, iterations);
    }
}

fn compress_file(file: &str, iterations: i32) {
    match read_file(file) {
        Ok(contents) => {
            let start_time = Instant::now();
            let optimized_contents =
                match optimise_file_contents(contents.clone(), iterations) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error compressing {}: {}", file, e);
                        return;
                    }
                };
            let elapsed_time = start_time.elapsed();

            if optimized_contents.len() < contents.len() {
                let saved_space = contents.len() - optimized_contents.len();
                if let Err(e) = write_file(file, optimized_contents) {
                    eprintln!("Error writing to {}: {}", file, e);
                } else {
                    println!(
                        "File {} compressed. Saved space: {} bytes. \nCompression time: {:?}",
                        file, saved_space, elapsed_time
                    );
                }
            } else {
                println!(
                    "File {} not compressed. No space saved. \nCompression time: {:?}",
                    file, elapsed_time
                );
            }
        }
        Err(e) => eprintln!("Error reading from {}: {}", file, e),
    }
}

fn parse_arg(arg: &str, args: &Vec<String>, index: usize) -> std::result::Result<i32, String> {
    if arg.starts_with("-i") {
        args[index][2..].parse().map_err(|e| format!("Failed to parse iterations: {}", e))
    } else if arg.starts_with("--iterations") {
        args[index][12..].parse().map_err(|e| format!("Failed to parse iterations: {}", e))
    } else {
        Err("Invalid argument for parse_arg".to_string())
    }
}

fn read_file(path: &str) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    Ok(contents)
}

fn write_file(path: &str, contents: Vec<u8>) -> Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(&contents)?;
    Ok(())
}

fn optimise_file_contents(input: Vec<u8>, force_iterations: i32) -> Result<Vec<u8>> {
    let contents = match decompress(input.clone()) {
        Ok(c) => c,
        Err(e) => return Err(e)
    };

    let iter = if force_iterations != -1 {
        force_iterations
    } else if contents.len() > 20_000 {
        100
    } else {
        500
    };

    Ok(compress(contents, iter as u64).unwrap_or_else(|_| input))
}

fn decompress(stuff: Vec<u8>) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(Cursor::new(stuff.clone()));
    let mut result = Vec::new();

    match decoder.read_to_end(&mut result) {
        Ok(_) => Ok(result),
        Err(e) => Err(e)
    }
}

fn compress(stuff: Vec<u8>, iter: u64) -> Result<Vec<u8>> {
    let options = zopfli::Options {
        iteration_count: NonZeroU64::new(iter).unwrap(),
        ..Default::default()
    };

    let mut output = Vec::with_capacity(stuff.len());
    match zopfli::compress(options, Gzip, &stuff[..], &mut output) {
        Ok(_) => {
            output.shrink_to_fit();
            Ok(output)
        },
        Err(e) => Err(e)
    }
}
