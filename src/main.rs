use std::{
    cmp::{max, min},
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{self, Duration, SystemTime, UNIX_EPOCH},
};

use pixels::{Pixels, SurfaceTexture};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

const COUNTS_3V3: f64 = 1912.;
const COUNTS_GND: f64 = 160.;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 300;

fn main() {
    println!("Hello, world!");

    let time_per_screen: Duration = Duration::from_secs_f64(1. / 490.);

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

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        let scaled_size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);
        WindowBuilder::new()
            .with_title("Oscilloscope Client")
            .with_inner_size(scaled_size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
    };

    let mut last_draw = SystemTime::now();
    let mut last_column_index = 0;
    let mut reading_buf = [0; WIDTH as usize];

    event_loop.run(move |event, _, control_flow| {
        if let Event::RedrawRequested(_) = event {
            let frame = pixels.frame_mut();

            let mut readings_handle = readings_vec_clone.lock().unwrap();
            let mut readings = Vec::with_capacity(readings_handle.len());
            readings.append(readings_handle.as_mut());
            *readings_handle = Vec::new();
            drop(readings_handle);

            let current_time = SystemTime::now();
            let delta_time = current_time
                .duration_since(last_draw)
                .unwrap_or(Duration::ZERO);
            last_draw = current_time;

            let pixels_to_draw = max(
                (delta_time.as_secs_f64() / time_per_screen.as_secs_f64() * f64::from(WIDTH))
                    as usize,
                readings.len(),
            );

            let readings_per_pixel = max(readings.len() / pixels_to_draw, 1);

            let mut current_pixel = 0;

            while current_pixel < pixels_to_draw {
                let begin = current_pixel * readings_per_pixel;
                let end = min(begin + readings_per_pixel, readings.len());
                let slice = &readings[begin..end];
                let val = slice
                    .iter()
                    .fold(0., |acc, cur| acc + f64::from(cur.to_owned()))
                    / (slice.len() as f64);

                let yval = HEIGHT - (val / 4096. * HEIGHT as f64) as u32;

                reading_buf[last_column_index] = yval;
                current_pixel += 1;
                last_column_index += 1;
                last_column_index %= WIDTH as usize;
            }

            for (i, p) in frame.chunks_exact_mut(4).enumerate() {
                let x = i % WIDTH as usize;
                let y = i / WIDTH as usize;

                if y == reading_buf[x].try_into().unwrap_or(0) {
                    p.copy_from_slice(&[0x00, 0xff, 0x00, 0xff])
                } else {
                    p.copy_from_slice(&[0x00, 0x00, 0x00, 0xff])
                }
            }

            if let Err(err) = pixels.render() {
                println!("error: {:?}", err);
                *control_flow = ControlFlow::Exit;
                return;
            }

            window.request_redraw();
        }

        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }
    });
}
