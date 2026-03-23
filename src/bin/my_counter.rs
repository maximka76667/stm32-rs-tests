#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;

use embassy_executor::Spawner;
use embassy_stm32::bind_interrupts;
use embassy_stm32::exti::{self, ExtiInput};
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::interrupt;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::Timer;
use fmt::info;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

static COUNTER: Mutex<ThreadModeRawMutex, u32> = Mutex::new(0);
static IS_FAST: Mutex<ThreadModeRawMutex, bool> = Mutex::new(false);

static SIGNAL: Signal<ThreadModeRawMutex, ()> = Signal::new();

#[embassy_executor::task]
async fn logger_task() {
    info!("Logger task ready!");

    loop {
        {
            let counter = COUNTER.lock().await;
            info!("Counter value: {}", *counter);
        }
        Timer::after_secs(2).await;
    }
}

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) {
    info!("Button task ready!");

    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");

        {
            let mut is_fast = IS_FAST.lock().await;
            *is_fast = !*is_fast;
        }

        SIGNAL.signal(());

        button.wait_for_falling_edge().await;
        info!("Released!");
    }
}
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    info!("Starting the program!");

    let mut red_led = Output::new(p.PB14, Level::Low, Speed::Low);
    let mut green_led = Output::new(p.PB0, Level::Low, Speed::Low);
    info!("LEDs created!");

    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(logger_task()).unwrap();

    let mut is_turned_on = false;

    loop {
        let delay = {
            let is_fast = IS_FAST.lock().await;
            if *is_fast { 300 } else { 1000 }
        };

        if is_turned_on {
            red_led.set_high();
            green_led.set_high();
        } else {
            red_led.set_low();
            green_led.set_low();
        }

        let signal_race =
            embassy_futures::select::select(Timer::after_millis(delay), SIGNAL.wait()).await;

        match signal_race {
            embassy_futures::select::Either::First(_) => (),
            embassy_futures::select::Either::Second(_) => continue,
        }

        is_turned_on = !is_turned_on;

        {
            let mut counter = COUNTER.lock().await;
            *counter += 1;
        }
    }
}
