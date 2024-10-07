use clap::Parser;
use console::Term;
use image::codecs::gif::GifDecoder;
use image::imageops::{flip_horizontal, flip_vertical, resize, rotate90};
use image::{AnimationDecoder, Rgba};
use std::error::Error;
use std::io::{self, BufRead, Cursor, Write};
use std::net::TcpStream;
use std::sync::mpsc::channel;

#[derive(Copy, Clone)]
struct Coordinates {
    x: u16,
    y: u16,
    bounds: (u16, u16),
}

impl std::ops::Add<Coordinates> for Coordinates {
    type Output = Coordinates;

    fn add(self, other: Coordinates) -> Coordinates {
        Coordinates {
            x: (self.x + other.x + self.bounds.0) % self.bounds.0,
            y: (self.y + other.y + self.bounds.1) % self.bounds.1,
            bounds: self.bounds,
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

enum Direction {
    Right,
    Left,
    Up,
    Down,
}

fn write_frame_to_stream<T: Write>(
    frame: &image::RgbaImage,
    position: Coordinates,
    buffer: &mut T,
    canvas_size: (u16, u16),
) -> io::Result<()> {
    for (x, y, &color) in frame.enumerate_pixels() {
        Pixel {
            point: Coordinates {
                x: x as u16,
                y: y as u16,
                bounds: canvas_size,
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

    let pacman_size: u32 = 60;

    let (direction_tx, direction_rx) = channel();

    let direction_tx_socket = direction_tx.clone();

    let _input_thread = std::thread::spawn(move || {
        let term = Term::stdout();
        loop {
            // Read character
            let c = term.read_char().expect("Failed to read input");
            let direction = match c {
                'w' => Direction::Up,
                'a' => Direction::Left,
                's' => Direction::Down,
                'd' => Direction::Right,
                _ => continue,
            };
            direction_tx
                .send(direction)
                .expect("Failed to move keypress to main thread");
        }
    });

    let _input_socket_thread = std::thread::spawn(move || {
        let listener =
            std::net::TcpListener::bind("0.0.0.0:1234").expect("Failed to bind to socket");
        let mut connection_pool = Vec::new();
        for stream in listener.incoming() {
            let stream = stream.expect("Failed to get stream");
            println!(
                "Remote control connected. (IP: {} | Connection: {})",
                stream.peer_addr().unwrap(),
                connection_pool.len()
            );
            let tx_handle = direction_tx_socket.clone();
            connection_pool.push(std::thread::spawn(move || {
                let mut reader =
                    io::BufReader::new(stream.try_clone().expect("Failed to clone stream"));
                loop {
                    let mut buffer = String::new();
                    reader
                        .read_line(&mut buffer)
                        .expect("Failed to read the server response from the stream");

                    // Break if the connection is closed
                    if buffer.is_empty() {
                        println!(
                            "Remote control disconnected! (IP: {})",
                            stream.peer_addr().unwrap()
                        );
                        break;
                    }

                    let direction = match buffer.trim() {
                        "w" => Direction::Up,
                        "a" => Direction::Left,
                        "s" => Direction::Down,
                        "d" => Direction::Right,
                        _ => continue,
                    };
                    tx_handle
                        .send(direction)
                        .expect("Failed to move socket input to main thread");
                }
            }));
        }
    });

    let gif_decoder =
        GifDecoder::new(Cursor::new(include_bytes!("pac.gif"))).expect("Failed to decode gif file");

    let right_frames = gif_decoder
        .into_frames()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to decode gif into frames")
        .iter()
        .map(|frame| {
            resize(
                &frame.clone().into_buffer(),
                pacman_size,
                pacman_size,
                image::imageops::FilterType::Nearest,
            )
        })
        .collect::<Vec<_>>();
    let left_frames = right_frames.iter().map(flip_horizontal).collect::<Vec<_>>();
    let down_frames = right_frames.iter().map(rotate90).collect::<Vec<_>>();
    let up_frames = down_frames.iter().map(flip_vertical).collect::<Vec<_>>();

    // Create a connection to the server
    let connection = TcpStream::connect(&args.url)?;

    let canvas_size = get_canvas_size(&connection);

    let mut buff_writer = io::BufWriter::new(connection);

    let frame_duration = 200;

    let mut position = Coordinates {
        x: args.x,
        y: args.y,
        bounds: canvas_size,
    };

    let start_time = std::time::Instant::now();
    let mut direction = Direction::Right;

    loop {
        // Check if there is a new direction
        if let Ok(new_direction) = direction_rx.try_recv() {
            direction = new_direction;
        }

        position = match direction {
            Direction::Right => {
                position.x += 1;
                position
            }
            Direction::Left => {
                position.x -= 1;
                position
            }
            Direction::Up => {
                position.y -= 1;
                position
            }
            Direction::Down => {
                position.y += 1;
                position
            }
        };

        let current_frames = match direction {
            Direction::Right => &right_frames,
            Direction::Left => &left_frames,
            Direction::Up => &up_frames,
            Direction::Down => &down_frames,
        };

        for _ in 0..10 {
            let elapsed_time = (std::time::Instant::now() - start_time).as_millis();
            let frame_idx: usize = (elapsed_time / frame_duration) as usize % current_frames.len();
            write_frame_to_stream(
                &current_frames[frame_idx],
                position,
                &mut buff_writer,
                canvas_size,
            )?;
        }
    }
}
