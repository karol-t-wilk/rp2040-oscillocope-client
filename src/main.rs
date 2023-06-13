use std::{
    cmp::{max, min},
    env,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime},
};

use minifb::{Window, WindowOptions, Scale, Key, KeyRepeat};

const WIDTH: usize = 800;
const HEIGHT: usize = 500;

fn main() {
    let num: u64 = env::args()
        .nth(1)
        .expect("Must give duration")
        .parse()
        .expect("Wrong format for duration");
    let unit = env::args().nth(2).expect("Must give unit");

    let mut time_per_screen: Duration;

    match unit.as_str() {
        "s" => time_per_screen = Duration::from_secs(num),
        "ms" => time_per_screen = Duration::from_millis(num),
        "us" => time_per_screen = Duration::from_micros(num),
        _ => panic!("Unsupported unit!")
    }


    let device = rusb::devices()
        .unwrap()
        .iter()
        .find(|device| {
            let device_desc = device.device_descriptor().unwrap();
            println!(
                "{:x} {:x}",
                device_desc.vendor_id(),
                device_desc.product_id()
            );
            return device_desc.vendor_id() == 0x16c0 && device_desc.product_id() == 0x27dd;
        })
        .unwrap();

    let handle = device.open().unwrap();

    let mut ep_addr = None;

    for i in 0..=255 {
        let mut buf = [0 as u8; 64];
        match handle.read_bulk(i, &mut buf, Duration::from_secs(5)) {
            Ok(_) => {
                println!("Found open endpoint {}", i);
                ep_addr = Some(i)
            }
            Err(_) => {}
        }
    }

    if ep_addr.is_none() {
        panic!("No open endpoints!")
    }

    let readings_vec = Arc::new(Mutex::new(Vec::new()));
    let readings_vec_clone = readings_vec.clone();

    thread::spawn(move || {
        let mut buf = [0 as u8; 64];
        loop {
            match handle.read_bulk(ep_addr.unwrap(), &mut buf, Duration::from_secs(5)) {
                Ok(size) => {
                    let num_readings = size / 2;
                    let mut readings_handle = readings_vec.lock();
                    for i in 0..num_readings {
                        let first_byte_index = i * 2;
                        let second_byte_index = first_byte_index + 1;
                        let reading =
                            (buf[first_byte_index] as u16) << 8 | (buf[second_byte_index] as u16);
                        if let Ok(h) = &mut readings_handle {
                            (*h).push(reading);
                        }
                    }
                }
                Err(err) => println!("Error: {}", err),
            }
        }
    });

    let mut window = Window::new("Oscilloscope Client", WIDTH, HEIGHT, WindowOptions {
        scale: Scale::X2,
        ..WindowOptions::default()
    }).unwrap();
    let mut window_buffer = [0u32; WIDTH * HEIGHT];

    let mut last_draw = SystemTime::now();
    let mut last_column_index = 0;
    let mut reading_buf = [0; WIDTH as usize];

    let readings_num = Arc::new(Mutex::new(0));
    let readings_num_clone = readings_num.clone();

    thread::spawn(move || loop {
        println!("rate = {}", std::mem::replace(&mut *readings_num_clone.lock().unwrap(), 0));
        thread::sleep(Duration::from_secs(1));
    });

    let mut is_paused = false;
    let mut average_readings = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let readings = std::mem::replace(&mut *readings_vec_clone.lock().unwrap(), Vec::new());

        *readings_num.lock().unwrap() += readings.len();

        let current_time = SystemTime::now();
        let delta_time = current_time
            .duration_since(last_draw)
            .unwrap_or(Duration::ZERO);

        let pixels_to_draw = ((delta_time.as_secs_f64() / time_per_screen.as_secs_f64()
            * WIDTH as f64) as usize)
            .clamp(1, readings.len());
        //let pixels_to_draw = readings.len();

        //println!("min/max {:?}/{:?}", readings.iter().min(), readings.iter().max());

        let readings_per_pixel = max(readings.len() / pixels_to_draw, 1);

        let mut current_pixel = 0;

        while current_pixel < pixels_to_draw {
            let val = if average_readings {
                let start = min(current_pixel * readings_per_pixel, readings.len() - 1);
                let end = min(start + readings_per_pixel, readings.len());
                let slice = &readings[start..end];
                slice
                    .iter()
                    .fold(0., |acc, cur| acc + f64::from(cur.to_owned()))
                    / (slice.len() as f64)
            } else {
                f64::from(readings[min(current_pixel * readings_per_pixel, readings.len() - 1)])
            };

            let yval = (HEIGHT - (val / 4096. * HEIGHT as f64) as usize).clamp(0, HEIGHT - 1);

            reading_buf[(last_column_index + current_pixel) % WIDTH] = yval;
            current_pixel += 1;
        }

        last_column_index = (last_column_index + pixels_to_draw) % WIDTH as usize;

        if !is_paused {
            for (i, p) in window_buffer.iter_mut().enumerate() {
                let x = i % WIDTH as usize;
                let y = i / WIDTH as usize;

                if y == reading_buf[x] as usize {
                    *p = 0x0000ff00;
                } else {
                    *p = 0x00000000;
                }
            }
        }

        window.update_with_buffer(&window_buffer, WIDTH, HEIGHT).unwrap();

        last_draw = current_time;

        if window.is_key_down(Key::Up) {
            time_per_screen += Duration::from_micros(100);
            println!("dur = {}", time_per_screen.as_micros());
        } else if window.is_key_down(Key::Down) {
            time_per_screen = max(
                Duration::from_micros(100),
                time_per_screen - Duration::from_micros(100),
            );
            println!("dur = {}", time_per_screen.as_micros());
        } else if window.is_key_pressed(Key::P, KeyRepeat::No) {
            is_paused = !is_paused;
        } else if window.is_key_pressed(Key::A, KeyRepeat::No) {
            average_readings = !average_readings;
        }
    }
}
