#![no_std]
#![no_main]

mod fmt;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::Timer;
use fmt::info;

enum State {
    Green,
    Yellow,
    Red,
    Reset,
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let mut red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    let mut yellow_led = Output::new(p.PE1, Level::Low, Speed::Low);
    let mut green_led = Output::new(p.PB0, Level::Low, Speed::Low);

    info!("Starting the program!");

    let mut state = State::Green;

    loop {
        state = match state {
            State::Green => {
                green_led.set_high();
                Timer::after_millis(1000).await;
                State::Yellow
            }
            State::Yellow => {
                yellow_led.set_high();
                Timer::after_millis(1000).await;
                State::Red
            }
            State::Red => {
                red_led.set_high();
                Timer::after_millis(1000).await;
                State::Reset
            }
            State::Reset => {
                green_led.set_low();
                red_led.set_low();
                yellow_led.set_low();
                Timer::after_millis(1000).await;
                State::Green
            }
        }
    }
}
