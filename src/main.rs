use std::net::TcpStream;
use std::io::Write;
use std::io;


struct Coordinates {
    x: u16,
    y: u16,
}

struct Color(u8, u8, u8);

impl Color {
    // String representation of the color (hex values of the RGB components)
    fn to_string(&self) -> String {
        format!("{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }
}

struct Pixel {
    point: Coordinates,
    rgb: Color,
}

impl Pixel {
    // String representation of the pixel (hex values of the RGB components)
    fn to_string(&self) -> String {
        format!("PX {} {} {}\n", self.point.x, self.point.y, self.rgb.to_string())
    }
}

fn generate_pixel_string(pixels: &[Pixel]) -> String {
    let mut pixel_string = String::new();
    for pixel in pixels {
        pixel_string.push_str(&pixel.to_string());
    }
    pixel_string
}

fn black_square() -> Vec<Pixel> {
    let mut pixels = Vec::new();
    for x in 0..1000 {
        for y in 0..1000 {
            pixels.push(Pixel {
                point: Coordinates { x, y },
                rgb: Color(0, 0, 0),
            });
        }
    }
    pixels
}

fn send_pixels(ip: &str, pixels: &[Pixel]) -> io::Result<()> {
    // Generate the string to send to the server
    let pixel_string = generate_pixel_string(pixels);

    // Send the string to the server
    let mut stream = TcpStream::connect(ip)?;
    println!("Successfully connected to {}", ip);


    // Send the string to the server
    stream.write(pixel_string.as_bytes())?;

    Ok(())
}


fn main() {
    println!("Start pixel client");

    let ip = "[fc42::1]:1234";

    // Generate a black square of 1000x1000 pixels
    let pixels = black_square();

    // Send the pixels to the server
    match send_pixels(ip, &pixels) {
        Ok(_) => {
            println!("Successfully sent pixels");
        }
        Err(e) => {
            println!("Failed to send pixels: {}", e);
        }
    }
}
