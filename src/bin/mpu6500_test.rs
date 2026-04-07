#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;
use fmt::info;

use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts, dma,
    i2c::{self, I2c},
    peripherals,
};

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

const ADDRESS: u8 = 0x68;
const WHOAMI: u8 = 0x75;

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

    let mut errors = 0u32;
    let mut success = 0u32;

    for _ in 0..10000u32 {
        let mut buf = [0u8; 1];
        match i2c.blocking_write_read(ADDRESS, &[WHOAMI], &mut buf) {
            Ok(()) if buf[0] == 0x70 => success += 1,
            _ => errors += 1,
        }
    }

    info!("Success: {}, Errors: {}", success, errors);
}
