#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;

use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts, dma,
    i2c::{self, I2c},
    peripherals,
};
use fmt::info;

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

    let mut i2c = I2c::new(
        p.I2C2,
        p.PB10,
        p.PB11,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Irqs,
        Default::default(),
    );

    info!("Starting I2C scan...");

    for addr in 0x08..0x78u8 {
        let mut buf = [0u8; 1];
        match i2c.blocking_read(addr, &mut buf) {
            Ok(_) => info!("Found device at: {:#X}", addr),
            Err(_) => {} // no device at this address, keep scanning
        }
    }

    info!("Scan complete.");
}
