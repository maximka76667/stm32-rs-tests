#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;

use core::hint::black_box;
use embassy_stm32::{
    bind_interrupts,
    exti::{self, ExtiInput},
    gpio::{Level, Output, Pull, Speed},
    interrupt,
    mode::Async,
    wdg::IndependentWatchdog,
};
use fmt::info;

use embassy_time::Timer;
#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

use embassy_executor::Spawner;

bind_interrupts!(
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static, Async>) {
    info!("Button task ready!");

    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");

        button.wait_for_falling_edge().await;
        info!("Released!");
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let mut red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    let mut green_led = Output::new(p.PB0, Level::Low, Speed::Low);
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);

    spawner.spawn(button_task(button)).unwrap();

    let mut watchdog = IndependentWatchdog::new(p.IWDG1, 3_000_000);

    green_led.set_high();
    Timer::after_millis(1000).await;
    green_led.set_low();
    watchdog.unleash();

    loop {
        red_led.set_high();
        Timer::after_millis(2000).await;
        red_led.set_low();

        for i in 0..u32::MAX {
            black_box(i);
            // busy wait, watchdog will fire here
        }

        watchdog.pet();
        info!("Watchdog restarted");
    }
}
