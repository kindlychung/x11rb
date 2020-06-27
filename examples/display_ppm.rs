use x11rb::connection::Connection;
use x11rb::errors::ReplyOrIdError;
use x11rb::image::Image;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{AtomEnum, CreateWindowAux, ConnectionExt, CreateGCAux, PropMode, Screen, VisualClass, Visualid, Visualtype, Window, WindowClass};
use x11rb::wrapper::ConnectionExt as _;

const DEPTH: u8 = 24;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        WM_PROTOCOLS,
        WM_DELETE_WINDOW,
    }
}

/// Create a window with the given image as background.
fn create_window(conn: &impl Connection, screen: &Screen, atoms: &Atoms, image: &Image) -> Result<Window, ReplyOrIdError> {
    let win_id = conn.generate_id()?;
    let pixmap_id = conn.generate_id()?;
    let gc_id = conn.generate_id()?;

    conn.create_gc(gc_id, screen.root, &CreateGCAux::default().graphics_exposures(0))?;
    conn.create_pixmap(DEPTH, pixmap_id, screen.root, image.width(), image.height())?;
    image.put(conn, pixmap_id, gc_id, 0, 0)?;
    conn.free_gc(gc_id)?;

    conn.create_window(
        screen.root_depth,
        win_id,
        screen.root,
        0,
        0,
        image.width(),
        image.height(),
        0,
        WindowClass::InputOutput,
        0,
        &CreateWindowAux::default()
            .background_pixmap(pixmap_id)
    )?;
    conn.free_pixmap(pixmap_id)?;

    conn.change_property32(
        PropMode::Replace,
        win_id,
        atoms.WM_PROTOCOLS,
        AtomEnum::ATOM,
        &[atoms.WM_DELETE_WINDOW],
    )?;

    Ok(win_id)
}

/// Check that the given visual is "as expected" (pixel values are 0xRRGGBB with RR/GG/BB being the
/// colors). Otherwise, this exits the process.
fn check_visual(screen: &Screen, id: Visualid) {
    // Find the information about the visual and at the same time check its depth.
    let visual_type = screen
        .allowed_depths
        .iter()
        .find(|depth| depth.depth == DEPTH)
        .and_then(|depth| depth.visuals
             .iter()
             .find(|depth| depth.visual_id == id)
         );
    let visual_type = match visual_type {
        Some(visual_type) => visual_type,
        None => {
            eprintln!("The root visual does not have depth {}", DEPTH);
            std::process::exit(1);
        }
    };
    // Now check that the pixels have red/green/blue components that we can set directly.
    match visual_type.class {
        VisualClass::TrueColor | VisualClass::DirectColor => {},
        _ => {
            eprintln!("The root visual is not true / direct color, but {:?}", visual_type);
            std::process::exit(1);
        }
    }
    match visual_type {
        // Check that the pixel setup is the way we want: 0xRRGGBB
        Visualtype {
            bits_per_rgb_value: 8,
            red_mask: 0xff0000,
            green_mask: 0x00ff00,
            blue_mask: 0x0000ff,
            ..
        } => {},
        _ => {
            eprintln!("Unsupported visual type {:?}", visual_type);
            std::process::exit(1);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the image
    let image = match std::env::args_os().skip(1).next() {
        None => {
            eprintln!("Expected a file name of a PPM as argument, using a built-in default image instead");
            ppm_parser::parse_ppm_bytes(&BUILTIN_IMAGE)?
        }
        Some(arg) => ppm_parser::parse_ppm_file(&arg)?,
    };

    let (conn, screen_num) = x11rb::connect(None)?;

    // The following is only needed for start_timeout_thread(), which is used for 'tests'
    let conn1 = std::sync::Arc::new(conn);
    let conn = &*conn1;

    let screen = &conn.setup().roots[screen_num];
    check_visual(screen, screen.root_visual);

    // Convert the image into the server's native format.
    let image = image.native(conn.setup())?;

    let atoms = Atoms::new(conn)?.reply()?;
    let win_id = create_window(conn, screen, &atoms, &image)?;
    conn.map_window(win_id)?;

    util::start_timeout_thread(conn1.clone(), win_id);

    conn.flush()?;

    loop {
        let event = conn.wait_for_event().unwrap();
        match event {
            Event::ClientMessage(event) => {
                let data = event.data.as_data32();
                if event.format == 32 && event.window == win_id && data[0] == atoms.WM_DELETE_WINDOW {
                    println!("Window was asked to close");
                    return Ok(());
                }
            }
            Event::Error(err) => println!("Got an unexpected error: {:?}", err),
            ev => println!("Got an unknown event: {:?}", ev),
        }
    }
}

mod ppm_parser {
    use std::io::{Error as IOError, ErrorKind, Read, Result as IOResult};
    use std::ffi::OsStr;

    use x11rb::image::{BitsPerPixel, Image, ScanlinePad};
    use x11rb::protocol::xproto::ImageOrder;

    fn make_io_error(text: &'static str) -> IOError {
        IOError::new(ErrorKind::Other, text)
    }

    /// Read until the next b'\n'.
    fn read_to_end_of_line(input: &mut impl Read) -> IOResult<()> {
        let mut byte = [0; 1];
        loop {
            input.read_exact(&mut byte)?;
            if byte[0] == b'\n' {
                return Ok(())
            }
        }
    }

    /// Read a decimal number from the input.
    fn read_decimal(input: &mut impl Read) -> IOResult<u16> {
        let mut byte = [0; 1];

        // Skip leading whitespace and comments
        loop {
            input.read_exact(&mut byte)?;
            match byte[0] {
                b' ' | b'\t' | b'\r' => {},
                // Comment, skip a whole line
                b'#' => read_to_end_of_line(input)?,
                _ => break,
            }
        }

        // Now comes a number
        if !byte[0].is_ascii_digit() {
            return Err(make_io_error("Failed parsing a number"));
        }

        let mut result: u16 = 0;
        while byte[0].is_ascii_digit() {
            let value = u16::from(byte[0] - b'0');
            result = result.checked_mul(10)
                .map(|result| result + value)
                .ok_or(make_io_error("Overflow while parsing number"))?;

            input.read_exact(&mut byte)?;
        }

        // After the number, there should be some whitespace.
        if byte[0].is_ascii_whitespace() {
            Ok(result)
        } else {
            Err(make_io_error("Unexpected character in header"))
        }
    }

    fn parse_ppm(input: &mut impl Read) -> IOResult<Image<'static>> {
        let mut header = [0; 2];
        input.read_exact(&mut header)?;
        if header != *b"P6" {
            return Err(make_io_error("Incorrect file header"));
        }
        read_to_end_of_line(input)?;
        let width = read_decimal(input)?;
        let height = read_decimal(input)?;
        let max = read_decimal(input)?;

        if max != 255 {
            eprintln!("Image declares a max pixel value of {}, but I expected 255.", max);
            eprintln!("Something will happen...?");
        }

        let mut image = Image::allocate(width, height, ScanlinePad::Pad8, 24, BitsPerPixel::B24, ImageOrder::MSBFirst);
        input.read_exact(image.data_mut())?;

        Ok(image)
    }

    pub fn parse_ppm_bytes(bytes: &[u8]) -> IOResult<Image<'static>> {
        use std::io::Cursor;

        parse_ppm(&mut Cursor::new(bytes))
    }

    pub fn parse_ppm_file(file_name: &OsStr) -> IOResult<Image<'static>> {
        use std::fs::File;
        use std::io::BufReader;

        parse_ppm(&mut BufReader::new(File::open(file_name)?))
    }
}

// Simple builtin PPM that is used if none is provided on the command line
const BUILTIN_IMAGE: [u8; 35] = [
    b'P', b'6', b'\n',
    // width and height
    b'4', b' ', b'2', b'\n',
    b'2', b'5', b'5', b'\n',
    // Black pixel
    0x00, 0x00, 0x00,
    // red pixel
    0xff, 0x00, 0x00,
    // green pixel
    0x00, 0xff, 0x00,
    // blue pixel
    0x00, 0x00, 0xff,
    // white pixel
    0xff, 0xff, 0xff,
    // cyan pixel
    0x00, 0xff, 0xff,
    // magenta pixel
    0xff, 0x00, 0xff,
    // yellow pixel
    0xff, 0xff, 0x00,
];

include!("integration_test_util/util.rs");
