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

fn get_canvas_size(mut stream: &TcpStream) -> (u16, u16) {
    let mut reader = io::BufReader::new(stream.try_clone().expect("Failed to clone stream"));

    stream
        .write_all(b"SIZE\n")
        .expect("Failed to send size request to the server");

    let mut buffer = String::new();
    reader
        .read_line(&mut buffer)
        .expect("Failed to read the server response from the stream");

    let mut parts = buffer.split_whitespace();

    parts
        .next()
        .expect("Failed parsing of size response: Response is empty");

    let width = parts
        .next()
        .and_then(|f| f.parse::<u16>().ok())
        .expect("Failed parsing of size response");

    let height = parts
        .next()
        .and_then(|f| f.parse::<u16>().ok())
        .expect("Failed parsing of size response");

    (width, height)
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

    // Create a connection to the server
    let connection = TcpStream::connect(&args.url)?;

    let canvas_size = get_canvas_size(&connection);

    let mut buff_writer = io::BufWriter::new(connection);

    let movement_speed: u16 = 50; // Pixel per sec
    let animation_speed: u16 = 12; // FPS for the GIF

    let mut x_position_subpixel = args.x as f32;
    let mut last_time = std::time::Instant::now();

    let bar = ProgressBar::new(canvas_size.0 as u64);
    loop {
        let position = Coordinates {
            x: x_position_subpixel as u16,
            y: args.y,
        };

        for frame in gif_frames.iter() {
            let start_time = std::time::Instant::now();

            while start_time + std::time::Duration::from_secs_f32(1.0 / animation_speed as f32)
                > std::time::Instant::now()
            {
                write_frame_to_stream(frame, position, &mut buff_writer)?;
            }
        }
        let now = std::time::Instant::now();
        let delta_t = now - last_time;
        last_time = now;

        x_position_subpixel += movement_speed as f32 * delta_t.as_secs_f32();
        if position.x >= canvas_size.0 {
            x_position_subpixel = 0.0;
            bar.reset();
        } else {
            bar.set_position(position.x as u64);
        }
    }
}
