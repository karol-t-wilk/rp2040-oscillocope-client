use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use pixels::{SurfaceTexture, Pixels};
use winit::{event_loop::{EventLoop, ControlFlow}, dpi::LogicalSize, window::WindowBuilder, event::{Event, VirtualKeyCode}};
use winit_input_helper::WinitInputHelper;

const COUNTS_3V3: f64 = 1912.;
const COUNTS_GND: f64 = 160.;

const WIDTH: u32 = 400;
const HEIGHT: u32 = 300;

fn main() {
    println!("Hello, world!");

    let device = rusb::devices()
        .unwrap()
        .iter()
        .find(|device| {
            let device_desc = device.device_descriptor().unwrap();
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

    let readings_thread = thread::spawn(move || {
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

    let gui_thread = thread::spawn(move || {
        let event_loop = EventLoop::new();
        let mut input = WinitInputHelper::new();

        let window = {
            let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
            let scaled_size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);
            WindowBuilder::new()
                .with_title("Conway's Game of Life")
                .with_inner_size(scaled_size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };

        let mut pixels = {
            let window_size = window.inner_size();
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            Pixels::new(WIDTH, HEIGHT, surface_texture).unwrap()
        };

        event_loop.run(move |event, _, control_flow| {
            if let Event::RedrawRequested(_) = event {

                if let Err(err) = pixels.render() {
                    println!("error: {:?}", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }

            if input.update(&event) {
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
        })
    });

    readings_thread.join().unwrap();
    gui_thread.join().unwrap();
}
