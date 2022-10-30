extern crate libusb;

use std::fs;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use std::time::Duration;
use std::thread;
use tokio::time::sleep;
use tokio::sync::mpsc;
use clap::Parser;
use clap_num::maybe_hex;

// Constants from /usr/include/linux/input-event-codes.h
const EV_KEY: u16 = 0x01;
const KEY_LEFT_META: u16 = 125;
const KEY_RIGHT_META: u16 = 125;
const KEY_L: u16 = 38;

#[derive(Parser)]
#[command(version, about = "Controls the dimming of the keyboard backlight", long_about = None)]
struct Cli {
    /// The USB Vendor ID of the controller
    #[arg(short, long, value_parser=maybe_hex::<u16>, default_value_t=1165)]
    vendor_id: u16,
    /// The USB Product ID of the controller
    #[arg(short, long, value_parser=maybe_hex::<u16>)]
    product_id: u16,
    /// The number of seconds to wait after a keypress before dimming
    #[arg(short, long, default_value_t = 5.0)]
    timeout: f64,
    /// Whether to dim the keyboard when Meta+L is pressed
    #[arg(short, long)]
    lock: bool
}


// Spawns a sleep
async fn create_timeout(duration: Duration) {
    tokio::spawn(sleep(duration)).await.unwrap();
}


// Determines which device under /dev/input is the keyboard and returns that
// path
fn get_keyboard_event() -> Result<String, String> {
    // Read dir listing of /sys/class/input
    let entries = match fs::read_dir("/sys/class/input") {
        Ok(e) => e,
        Err(e) => return Err(e.to_string())
    };

    // Iterate over entries
    for path in entries {
        if let Ok(path) = path {
            // Only search for event*
            if !path.file_name().to_str().unwrap().starts_with("event") {
                continue;
            }

            // Get the path to the device name file
            let name_path = Path::new("/sys/class/input").join(path.file_name()).join("device/name");
            let name_path_str = match name_path.to_str() {
                Some(e) => e,
                None => continue
            };


            // Open the file
            let mut file = match File::open(name_path_str) {
                Ok(file) => file,
                Err(_) => continue
            };

            // Read the contents
            let mut contents = String::new();
            match file.read_to_string(&mut contents) {
                Ok(_) => (),
                Err(_) => continue
            }

            // Check the contents
            if contents.contains("keyboard") {
                let result = Path::new("/dev/input").join(path.file_name());
                match result.to_str() {
                    Some(e) => return Ok(String::from(e)),
                    None => continue
                };
            }
        }
    }

    return Err(String::from("not found"))
}


// Takes control of a USB device and interface
fn take_control(handle: &mut libusb::DeviceHandle) -> bool {
    let is_active = match handle.kernel_driver_active(1) {
        Ok(a) => a,
        Err(e) => {
            println!("Error determining driver activity: {}", e);
            return false;
        }
    };

    if is_active {
        match handle.detach_kernel_driver(1) {
            Err(e) => {
                println!("Error detaching kernel driver: {}", e);
                return false;
            },
            _ => {
                return true;
            }
        }
    } else {
        return false;
    }
}


// Releases control of a USB device and interface if it was taken
fn release_control(handle: &mut libusb::DeviceHandle, is_active: bool) {
    match handle.release_interface(1) {
        Err(e) => println!("Release Error: {}", e),
        _ => ()
    }

    if is_active {
        match handle.attach_kernel_driver(1) {
            Err(e) => println!("Error attaching kernel driver: {}", e),
            _ => ()
        }
    }
}


// Determines the current brightness level of the keyboard backlight
fn read_brightness_level(handle: &mut libusb::DeviceHandle) -> Result<u8, String> {
    let is_active = take_control(handle);

    // 0x88 is "get effect"
    // 0x02 is "effect attribute brightness"
    let mut data: [u8; 8] = [0x88, 0x02, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00];
    match handle.claim_interface(1) {
        Err(e) => {
            return Err(e.to_string());
        },
        _ => ()
    }

    // Set up some request types
    let request_type_in = libusb::request_type(libusb::Direction::In, libusb::RequestType::Class, libusb::Recipient::Interface);
    let request_type_out = libusb::request_type(libusb::Direction::Out, libusb::RequestType::Class, libusb::Recipient::Interface);

    // Write out the request to read the brightness
    // request 0x09 is HID set_report
    // value 0x0300 is HID feature
    // index 0x0001 is whatever
    match handle.write_control(request_type_out, 0x09, 0x0300, 0x0001, &data, Duration::from_secs(1)) {
        Err(e) => {
            return Err(e.to_string());
        },
        _ => ()
    }

    // Read the brightness
    // request 0x01 is HID get_report
    // value 0x0300 is HID feature
    // index 0x0001 is whatever
    match handle.read_control(request_type_in, 0x01, 0x0300, 0x0001, &mut data, Duration::from_secs(1)) {
        Err(e) => {
            return Err(e.to_string());
        },
        _ => ()
    }

    release_control(handle, is_active);

    return Ok(data[4])
}


// Sets the keyboard backlight level
fn set_backlight_level(handle: &mut libusb::DeviceHandle, level: u8) {
    let is_active = take_control(handle);

    // 0x08 is "set effect"
    // 0x02 is "effect attribute brightness"
    let data: [u8; 8] = [0x08, 0x02, 0x33, 0x00, level, 0x00, 0x00, 0x00];
    match handle.claim_interface(1) {
        Err(e) => {
            println!("Claim Error: {}", e);
            return;
        },
        _ => ()
    }

    // Set up the request type
    let request_type = libusb::request_type(libusb::Direction::Out, libusb::RequestType::Class, libusb::Recipient::Interface);

    // request 0x09 is HID set_report
    // value 0x0300 is HID feature
    // index 0x0001 is whatever
    match handle.write_control(request_type, 0x09, 0x0300, 0x0001, &data, Duration::from_secs(1)) {
        Err(e) => println!("Error: {}", e),
        _ => ()
    }

    release_control(handle, is_active);
}


// Entry point
#[tokio::main(worker_threads=2)]
async fn main() {
    // Parse the command line arguments
    let args = Cli::parse();

    // Get the path to our keyboard input device
    let event_path = match get_keyboard_event() {
        Ok(e) => {
            println!("Found keyboard device at {}", e);
            e
        },
        Err(e) => panic!("couldn't find input device: {}", e)
    };

    // Initialise libusb
    let context = match libusb::Context::new() {
        Ok(context) => context,
        Err(e) => panic!("could not initialise libusb: {}", e)
    };

    // Open the USB device
    let mut handle = match context.open_device_with_vid_pid(args.vendor_id, args.product_id) {
        Some(handle) => {
            println!("Found matching USB device for vendor 0x{:04x}, product 0x{:04x}", args.vendor_id, args.product_id);
            handle
        },
        None => panic!("couldn't find USB device")
    };

    // Read the current brightness level
    let mut requested_level = match read_brightness_level(&mut handle) {
        Ok(l) => {
            println!("Startup Backlight Level: {}", l);
            l
        }
        Err(e) => {
            println!("Failed to get current brightness: {}", e);
            50
        }
    };

    // Create a thread that posts to a channel when it's able to read
    let (s, mut r) = mpsc::unbounded_channel();
    let thread_builder = thread::Builder::new().name("input-reader".to_string());
    let thread_start_result = thread_builder.spawn(move || {
        // Open input device
        let mut file = File::open(Path::new(&event_path)).expect("Failed to open input device");

        // Initialise a buffer large enough to read our input data
        let mut buf: [u8; 24] = [0; 24];

        // Debug
        println!("Input thread running");

        // Flag to keep track of Meta (Windows) key states
        let mut meta_l_down = false;
        let mut meta_r_down = false;

        // Read up to 24 bytes
        loop {
            let count = file.read(&mut buf).expect("Failed to read");
            if count < 24 {
                println!("Warning - too few bytes read");
            }

            // Parse the data to see what keys were pressed
            let in_type = (buf[17] as u16) << 8 | (buf[16] as u16);
            let code = (buf[19] as u16) << 8 | (buf[18] as u16);
            let value = (buf[23] as u32) << 24 | (buf[22] as u32) << 16 | (buf[21] as u32) << 8 | (buf[20] as u32);

            // Keep track if the Meta (Windows) key is down
            if in_type == EV_KEY {
                if code == KEY_LEFT_META {
                    meta_l_down = value > 0;
                } else if code == KEY_RIGHT_META {
                    meta_r_down = value > 0;
                }
            }

            // Check for a Meta+L combination key release
            let result = match in_type == EV_KEY && value == 0 && code == KEY_L && (meta_l_down || meta_r_down) {
                true => 1,
                false => 0
            };

            // Only send events on a key-up / key-down / key-repeat
            if in_type == EV_KEY {
                match s.send(result) {
                    Err(e) => println!("{}", e),
                    Ok(_) => ()
                }
            }
        }
    });
    match thread_start_result {
        Ok(_) => (),
        Err(e) => panic!("Failed to start input thread: {}", e)
    }

    // Turn the backlight on
    let mut level = requested_level;
    set_backlight_level(&mut handle, level);

    // Flag to indicate if we're currently dimming the backlight
    let mut dimming = false;

    // Flag to indicate if we currently think the backlight should be on (even
    // if it's at a requested level of zero)
    let mut is_active = true;

    // How many future key events to ignore
    let mut ignore_next = 0;

    // Loop forever
    loop {
        // Default to "dimming" timeout
        let mut timeout_time = 100;

        // If we're not dimming...
        if !dimming {
            // ...and we're inactive, set a long timeout
            if !is_active {
                timeout_time = 3600000;
            // ...and we're active, set the timeout to what the user requested
            } else {
                timeout_time = (args.timeout * 1000.0) as u64;
            }
        }

        // Set up our tasks
        let recv_task = r.recv();
        let timeout_task = create_timeout(Duration::from_millis(timeout_time));

        // Wait for one of the tasks to complete
        tokio::select! {
            // Keypress
            lock = recv_task => {
                // Ignore events if we're asked to
                if ignore_next > 0 {
                    ignore_next = ignore_next - 1;
                } else {
                    // If the result back was a lockscreen (and dim-on-locking is enabled)
                    if args.lock && lock.unwrap() == 1 {
                        // Ignore the next key event (so the Meta up doesn't trigger the backlight)
                        ignore_next = 1;

                        // Take us to dimming
                        dimming = true;
                    } else {
                        // Key was pressed, stop dimming, set active and change the
                        // backlight level if it's not current what the user set it to
                        is_active = true;
                        dimming = false;
                        if level != requested_level {
                            level = requested_level;
                            set_backlight_level(&mut handle, level);
                        }
                    }
                }
            },

            // Timeout
            _ = timeout_task => {
                // No key has been pressed recently, so we're definitely inactive
                // (and possibly already dimming)
                is_active = false;

                // If we're starting to dim...
                if !dimming {
                    // Read the current brightness level as the user may have
                    // changed it via the keyboard
                    match read_brightness_level(&mut handle) {
                        Ok(l) => {
                            requested_level = l;
                            level = l;
                        }
                        Err(e) => {
                            println!("Failed to get current brightness: {}", e);
                            requested_level = level;
                        }
                    };

                    // Flag up that we're currently dimming
                    if requested_level > 0 {
                        dimming = true;
                    }
                }

                // Update the backlight level
                if level != 0 {
                    // The brightness is non-linear so change the dimming speed
                    // based on the current level
                    if level >= 10 {
                        level = level - 2;
                    } else if level >= 1 {
                        level = level - 1;
                    }

                    // Change the level if we've got something valid
                    if level <= 50 {
                        set_backlight_level(&mut handle, level);
                    }

                    // If we've reached level zero, we can stop dimming
                    if level == 0 {
                        dimming = false;
                    }
                }
            }
        }
    }
}
