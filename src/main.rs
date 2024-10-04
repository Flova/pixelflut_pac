use clap::Parser;
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
use image::Rgba;
use std::io;
use std::io::Cursor;
use std::io::Write;
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

fn generate_pixel_string(pixels: &[Pixel]) -> Vec<u8> {
    // Allocate byte buffer with matching size
    let mut pixel_buffer = Vec::new();

    for pixel in pixels {
        // Mutable borrow string
        pixel
            .write(&mut pixel_buffer)
            .expect("Failed to write pixel");
    }
    pixel_buffer
}

fn serialize_frame(frame: &image::Frame, position: Coordinates) -> Vec<Pixel> {
    let mut pixels = Vec::with_capacity(frame.buffer().len());
    for (x, y, &pixel) in frame.buffer().enumerate_pixels() {
        pixels.push(Pixel {
            point: Coordinates {
                x: x as u16,
                y: y as u16,
            } + position,
            rgb: pixel.into(),
        });
    }
    pixels
}

fn send_pixels(ip: &str, pixels: &[Pixel]) -> io::Result<()> {
    // Generate the string to send to the server
    let pixel_string = generate_pixel_string(pixels);

    // Send the string to the server
    let mut stream = TcpStream::connect(ip)?;
    println!("Successfully connected to {}", ip);

    let recv_thread: std::thread::JoinHandle<()> = std::thread::spawn({
        let mut stream: TcpStream = stream.try_clone().unwrap();
        move || {
            std::io::copy(&mut stream, &mut std::io::stderr())
                .expect("Error sending server answer to stdout.");
        }
    });

    // Send the string to the server
    stream.write_all(pixel_string.as_slice())?;
    stream.shutdown(std::net::Shutdown::Write).unwrap();

    recv_thread.join().unwrap();

    Ok(())
}

fn main() {
    println!("Start pixel client");

    // Parse the command line arguments
    let args = Config::parse();

    let gif_decoder = GifDecoder::new(Cursor::new(include_bytes!("nyan.gif")))
        .expect("Failed to decode gif file");
    let gif_frames = gif_decoder
        .into_frames()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to decode gif into frames");

    let mut position = Coordinates {
        x: args.x,
        y: args.y,
    };

    loop {
        for frame in gif_frames.iter() {
            let frame = serialize_frame(frame, position);

            match send_pixels(&args.url, &frame) {
                Ok(_) => {
                    println!("Successfully sent pixels");
                }
                Err(e) => {
                    println!("Failed to send pixels: {}", e);
                }
            };
            position.x = (position.x + 15) % 1920
        }
    }
}
