use std::{time::{Duration, UNIX_EPOCH, SystemTime}};

const COUNTS_3V3: f64 = 1912.;
const COUNTS_GND: f64 = 160.;

fn main() {
    println!("Hello, world!");

    let device = rusb::devices().unwrap().iter().find(|device| {
        let device_desc = device.device_descriptor().unwrap();
        return device_desc.vendor_id() == 0x16c0 && device_desc.product_id() == 0x27dd;
    }).unwrap();

    let handle = device.open().unwrap();

    let mut ep_addr = None;

    for i in 0..=255 {
        let mut buf = [0 as u8; 64];
        match handle.read_bulk(i, &mut buf, Duration::from_secs(5)) {
            Ok(_) => {
                println!("Found open endpoint {}", i);
                ep_addr = Some(i)
            }
            Err(_) => {
            }
        }
    }

    if ep_addr.is_none() {
        panic!("No open endpoints!")
    }

    let mut last_report = UNIX_EPOCH;

    let mut readings_vec = Vec::new();

    let mut buf = [0 as u8; 64];
    loop {
        match handle.read_bulk(ep_addr.unwrap(), &mut buf, Duration::from_secs(5)) {
            Ok(size) => {
                let num_readings = size / 2;
                for i in 0..num_readings {
                    let first_byte_index = i * 2;
                    let second_byte_index = first_byte_index + 1;
                    let reading = (buf[first_byte_index] as u16) << 8 | (buf[second_byte_index] as u16);
                    readings_vec.push(reading);
                }
            },
            Err(err) => println!("Error: {}", err)
        }

        if SystemTime::now().duration_since(last_report).unwrap_or(Duration::ZERO) > Duration::from_secs(1) {
            last_report = SystemTime::now();
            let rate = readings_vec.len();
            let counts = readings_vec.iter().fold(0., |acc, cur| {acc + f64::from(cur.to_owned())}) / readings_vec.len() as f64;
            readings_vec.clear();

            let voltage =  (counts - COUNTS_GND) / (COUNTS_3V3 - COUNTS_GND) * 3.3;

            println!("Sample rate is {}, counts are {}, voltage is {}V", rate, counts, voltage)
        }
    }
}
