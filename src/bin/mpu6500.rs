#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use mpu6500_driver::Mpu6500;

#[path = "../fmt.rs"]
mod fmt;
use fmt::info;

use embassy_stm32::{
    bind_interrupts, dma,
    i2c::{self, I2c},
    peripherals,
};

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;
    DMA1_STREAM4 => dma::InterruptHandler<peripherals::DMA1_CH4>;
    DMA1_STREAM5 => dma::InterruptHandler<peripherals::DMA1_CH5>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let i2c = I2c::new(
        p.I2C2,
        p.PB10,
        p.PB11,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Irqs,
        Default::default(),
    );

    let mut sensor = Mpu6500::new(i2c);
    sensor.init().unwrap();

    info!("Calibrating... keep sensor still!");
    sensor.calibrate(1000).unwrap();
    info!("Done!");

    loop {
        let (ax, ay, az) = sensor.read_accel().unwrap();
        let (gx, gy, gz) = sensor.read_gyro().unwrap();

        info!("Accel: x={} y={} z={}", ax, ay, az);
        info!("Gyro:  x={} y={} z={}", gx, gy, gz);

        Timer::after_millis(1000).await;
    }
}
