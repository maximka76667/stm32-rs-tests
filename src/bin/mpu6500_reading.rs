#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;
use embassy_time::Timer;
use fmt::info;

use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts, dma,
    i2c::{self, I2c, Master},
    mode::Async,
    peripherals,
};

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

const ADDRESS: u8 = 0x68;
const WHOAMI: u8 = 0x75;

const PWR_MGMT_1: u8 = 0x6B;
const ACCEL_XOUT_H: u8 = 0x3B;
const GYRO_XOUT_H: u8 = 0x43;

bind_interrupts!(struct Irqs {
    I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;
    DMA1_STREAM4 => dma::InterruptHandler<peripherals::DMA1_CH4>;
    DMA1_STREAM5 => dma::InterruptHandler<peripherals::DMA1_CH5>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let mut i2c = I2c::new(
        p.I2C2,
        p.PB10,
        p.PB11,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Irqs,
        Default::default(),
    );

    // wake up sensor
    i2c.blocking_write(ADDRESS, &[PWR_MGMT_1, 0x00]).unwrap();

    let (
        mut i2c,
        (accel_x_offset, accel_y_offset, accel_z_offset),
        (gyro_x_offset, gyro_y_offset, gyro_z_offset),
    ) = calibrate(i2c);

    // read accelerometer - 6 bytes (2 bytes for each axis X, Y, Z)
    let mut accel_buf = [0u8; 6];

    loop {
        i2c.blocking_write_read(ADDRESS, &[ACCEL_XOUT_H], &mut accel_buf)
            .unwrap();

        let accel_x = i16::from_be_bytes([accel_buf[0], accel_buf[1]]) as i32 - accel_x_offset;
        let accel_y = i16::from_be_bytes([accel_buf[2], accel_buf[3]]) as i32 - accel_y_offset;
        let accel_z = i16::from_be_bytes([accel_buf[4], accel_buf[5]]) as i32 - accel_z_offset;

        info!("Accel: x={} y={} z={}", accel_x, accel_y, accel_z);

        // read gyroscope - 6 bytes (X, Y, Z, 2 bytes each)
        let mut gyro_buf = [0u8; 6];
        i2c.blocking_write_read(ADDRESS, &[GYRO_XOUT_H], &mut gyro_buf)
            .unwrap();

        let gyro_x = i16::from_be_bytes([gyro_buf[0], gyro_buf[1]]) as i32 - gyro_x_offset;
        let gyro_y = i16::from_be_bytes([gyro_buf[2], gyro_buf[3]]) as i32 - gyro_y_offset;
        let gyro_z = i16::from_be_bytes([gyro_buf[4], gyro_buf[5]]) as i32 - gyro_z_offset;

        info!("Gyro:  x={} y={} z={}", gyro_x, gyro_y, gyro_z);

        Timer::after_millis(1000).await;
    }
}

fn calibrate(
    mut i2c: I2c<'_, Async, Master>,
) -> (I2c<'_, Async, Master>, (i32, i32, i32), (i32, i32, i32)) {
    info!("Calibrating... keep sensor still!");

    let mut accel_x_sum: i32 = 0;
    let mut accel_y_sum: i32 = 0;
    let mut accel_z_sum: i32 = 0;
    let mut gyro_x_sum: i32 = 0;
    let mut gyro_y_sum: i32 = 0;
    let mut gyro_z_sum: i32 = 0;

    let samples = 1000i32;

    for _ in 0..samples {
        let mut accel_buf = [0u8; 6];
        let mut gyro_buf = [0u8; 6];

        i2c.blocking_write_read(ADDRESS, &[ACCEL_XOUT_H], &mut accel_buf)
            .unwrap();
        i2c.blocking_write_read(ADDRESS, &[GYRO_XOUT_H], &mut gyro_buf)
            .unwrap();

        accel_x_sum += i16::from_be_bytes([accel_buf[0], accel_buf[1]]) as i32;
        accel_y_sum += i16::from_be_bytes([accel_buf[2], accel_buf[3]]) as i32;
        accel_z_sum += i16::from_be_bytes([accel_buf[4], accel_buf[5]]) as i32;
        gyro_x_sum += i16::from_be_bytes([gyro_buf[0], gyro_buf[1]]) as i32;
        gyro_y_sum += i16::from_be_bytes([gyro_buf[2], gyro_buf[3]]) as i32;
        gyro_z_sum += i16::from_be_bytes([gyro_buf[4], gyro_buf[5]]) as i32;
    }

    let accel_x_offset = accel_x_sum / samples;
    let accel_y_offset = accel_y_sum / samples;
    // for Z we subtract gravity (16384 for right side up, -16384 for upside down)
    let accel_z_offset = accel_z_sum / samples - (-16384);
    let gyro_x_offset = gyro_x_sum / samples;
    let gyro_y_offset = gyro_y_sum / samples;
    let gyro_z_offset = gyro_z_sum / samples;

    info!("Calibration done!");
    info!(
        "Accel offsets: x={} y={} z={}",
        accel_x_offset, accel_y_offset, accel_z_offset
    );
    info!(
        "Gyro offsets:  x={} y={} z={}",
        gyro_x_offset, gyro_y_offset, gyro_z_offset
    );
    (
        i2c,
        (accel_x_offset, accel_y_offset, accel_z_offset),
        (gyro_x_offset, gyro_y_offset, gyro_z_offset),
    )
}
