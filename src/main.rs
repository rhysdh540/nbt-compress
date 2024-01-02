use std::io::{Result, Read, Write, Error, ErrorKind::InvalidData};
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use libdeflater::*;
use zopfli::Format::Gzip;


fn main() {
    let usage = "Usage: nbt-compress [-i <iterations>] file1 file2 ...";
    let args: Vec<String> = std::env::args().collect();
    let mut iterations = -1;
    let mut use_zopfli = false;
    let mut files = Vec::new();

    for (index, arg) in args.iter().enumerate() {
        if index == 0 {
            continue;
        }

        if arg == "-z" || arg == "--zopfli" {
            use_zopfli = true;
            continue;
        }

        if arg.starts_with("-") {
            match parse_arg(arg, &args, index) {
                Ok(i) => iterations = i,
                Err(e) => {
                    eprintln!("Error parsing argument: {}", e);
                    std::process::exit(1);
                }
            }
        } else {
            files.push(arg.clone());
        }
    }

    if files.is_empty() {
        println!("{}", usage);
        std::process::exit(1);
    }

    let mut total_time = Duration::new(0, 0);
    let mut total_saved_space = 0;

    for file in &files {
        match compress_file(file, iterations, use_zopfli) {
            Ok((elapsed_time, saved_space)) => {
                total_time += elapsed_time;
                total_saved_space += saved_space;
            }
            Err(_) => {}
        }
    }

    if files.len() > 1 {
        println!("\nDone!");
        println!("Total time: {:?}", total_time);
        println!("Total saved space: {} bytes", total_saved_space);
    }
}

fn compress_file(file: &str, iterations: i32, zopfli: bool) -> Result<(Duration, usize)> {
    match read_file(file) {
        Ok(contents) => {
            let start_time = Instant::now();
            let optimized_contents =
                match if zopfli { optimise_zopfli(contents.clone(), iterations) } else { compress_libdeflater(contents.clone(), 9) } {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error compressing {}: {}", file, e);
                        return Err(e);
                    }
                };

            let elapsed_time = start_time.elapsed();

            if optimized_contents.len() < contents.len() {
                let saved_space = contents.len() - optimized_contents.len();
                if let Err(e) = write_file(file, optimized_contents) {
                    eprintln!("Error writing to {}: {}", file, e);
                    Err(e)
                } else {
                    println!(
                        "File {} compressed. Saved space: {} bytes. \nCompression time: {:?}",
                        file, saved_space, elapsed_time
                    );
                    Ok((elapsed_time, saved_space))
                }
            } else {
                println!(
                    "File {} not compressed. No space saved. \nCompression time: {:?}",
                    file, elapsed_time
                );
                return Ok((elapsed_time, 0));
            }
        }
        Err(e) => {
            eprintln!("Error reading from {}: {}", file, e);
            Err(e)
        }
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

fn optimise_zopfli(input: Vec<u8>, force_iterations: i32) -> Result<Vec<u8>> {
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

    Ok(compress_zopfli(contents, iter as u64).unwrap_or_else(|_| input))
}

fn decompress(data: Vec<u8>) -> Result<Vec<u8>> {
    let mut decompressor = Decompressor::new();
    let mut dest = vec![0; data.len() * 2];
    loop {
        match decompressor.gzip_decompress(&*data, &mut dest) {
            Ok(len) => {
                dest.truncate(len);
                return Ok(dest);
            }
            Err(DecompressionError::InsufficientSpace) => {
                dest.resize(dest.len() * 2, 0);
            }
            Err(e) => return Err(Error::new(InvalidData, e)),
        }
    }
}
fn compress_libdeflater(data: Vec<u8>, level: u8) -> Result<Vec<u8>> {
    let mut compressor = Compressor::new(CompressionLvl::new(level.into()).unwrap());
    let capacity = compressor.gzip_compress_bound(data.len());
    let mut dest = vec![0; capacity];
    match compressor.gzip_compress(&*data, &mut dest) {
        Ok(len) => {
            dest.truncate(len);
            Ok(dest)
        }
        Err(e) => Err(Error::new(InvalidData, e)),
    }
}

fn compress_zopfli(stuff: Vec<u8>, iter: u64) -> Result<Vec<u8>> {
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
