use clap::Parser;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, Rgba};
use indicatif::ProgressBar;
use std::error::Error;
use std::io::{self, BufRead, Cursor, Write};
use std::net::TcpStream;

#[derive(Copy, Clone)]
struct Coordinates {
    x: u16,
    y: u16,
}

impl std::ops::Add<Coordinates> for Coordinates {
    type Output = Coordinates;

    fn add(self, other: Coordinates) -> Coordinates {
        Coordinates {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

#[derive(Copy, Clone)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl From<Rgba<u8>> for Color {
    fn from(rgba: Rgba<u8>) -> Self {
        Color {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
        }
    }
}

struct Pixel {
    point: Coordinates,
    rgb: Color,
}

impl Pixel {
    // Implement output function for buffer writing with a
    fn write<T: Write>(&self, buffer: &mut T) -> io::Result<()> {
        writeln!(
            buffer,
            "PX {x} {y} {r:02x}{g:02x}{b:02x}",
            x = self.point.x,
            y = self.point.y,
            r = self.rgb.r,
            g = self.rgb.g,
            b = self.rgb.b
        )?;
        Ok(())
    }
}

/// Command line that sends pixels to a pixelflut server
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Config {
    #[arg(short, long, default_value = "pixelflut:1234")]
    url: String,
    #[arg(default_value = "0")]
    x: u16,
    #[arg(default_value = "0")]
    y: u16,
}

fn write_frame_to_stream<T: Write>(
    frame: &image::Frame,
    position: Coordinates,
    buffer: &mut T,
) -> io::Result<()> {
    for (x, y, &color) in frame.buffer().enumerate_pixels() {
        Pixel {
            point: Coordinates {
                x: x as u16,
                y: y as u16,
            } + position,
            rgb: color.into(),
        }
        .write(buffer)?;
    }
    Ok(())
}

fn send_frame(ip: &str, frame: &image::Frame, position: Coordinates) -> io::Result<()> {
    // Send the string to the server
    let mut stream = TcpStream::connect(ip)?;

    let recv_thread: std::thread::JoinHandle<()> = std::thread::spawn({
        let mut stream: TcpStream = stream.try_clone().unwrap();
        move || {
            std::io::copy(&mut stream, &mut std::io::stderr())
                .expect("Error sending server answer to stdout.");
        }
    });

    write_frame_to_stream(frame, position, &mut stream)?;

    stream.shutdown(std::net::Shutdown::Write).unwrap();

    recv_thread.join().unwrap();

    Ok(())
}

fn get_canvas_size(ip: &str) -> Result<(u16, u16), &str> {
    let mut stream = TcpStream::connect(ip).map_err(|_| "Failed to connect to the server")?;

    let mut reader = io::BufReader::new(stream.try_clone().expect("Failed to clone stream"));

    stream
        .write_all(b"SIZE\n")
        .map_err(|_| "Failed to send SIZE request")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .expect("Failed to shutdown stream");

    let mut buffer = String::new();
    reader
        .read_line(&mut buffer)
        .map_err(|_| "Failed to read the server response from the stream")?;

    let mut parts = buffer.split_whitespace();

    parts.next().ok_or("Size response too short")?;

    let width = parts
        .next()
        .and_then(|f| f.parse::<u16>().ok())
        .ok_or("Failed parsing of size response")?;

    let height = parts
        .next()
        .and_then(|f| f.parse::<u16>().ok())
        .ok_or("Failed parsing of size response")?;

    Ok((width, height))
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Start pixel client");

    // Parse the command line arguments
    let args = Config::parse();

    let gif_decoder = GifDecoder::new(Cursor::new(include_bytes!("nyan.gif")))
        .expect("Failed to decode gif file");
    let gif_frames = gif_decoder
        .into_frames()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to decode gif into frames");

    let canvas_size = get_canvas_size(&args.url)?;

    let mut position = Coordinates {
        x: args.x,
        y: args.y,
    };

    let speed: u16 = 15; // Pixel per sec

    let bar = ProgressBar::new(canvas_size.0 as u64);
    loop {
        for frame in gif_frames.iter() {
            if let Err(e) = send_frame(&args.url, &frame, position) {
                eprintln!("Failed to send frame: {}", e);
            };
            position.x += speed;
            if position.x >= canvas_size.0 {
                position.x = 0;
                bar.reset();
            } else {
                bar.inc(speed as u64);
            }
        }
    }
}
