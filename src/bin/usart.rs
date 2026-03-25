#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;

use embassy_executor::Spawner;
use embassy_stm32::usart::{Config, Uart};

use fmt::{info, unwrap};

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let config = Config::default();
    let mut usart = Uart::new_blocking(p.UART7, p.PF6, p.PF7, config).unwrap();

    unwrap!(usart.blocking_write(b"Hello Embassy World!\r\n"));
    info!("wrote Hello, starting echo");

    let mut buf = [0u8; 1];
    loop {
        unwrap!(usart.blocking_read(&mut buf));
        unwrap!(usart.blocking_write(&buf));
    }
}
