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
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::Timer;
use fmt::{info, warn};

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(
    pub struct Irqs{
        EXTI15_10 => exti::InterruptHandler<interrupt::typelevel::EXTI15_10>;
});

static COUNTER: Mutex<ThreadModeRawMutex, u32> = Mutex::new(0);

static BUTTON_PRESSED: PubSubChannel<ThreadModeRawMutex, u32, 1, 2, 1> = PubSubChannel::new();

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
async fn watchdog_task() {
    info!("Watchdog task ready!");

    let mut sub = BUTTON_PRESSED.subscriber().unwrap();

    loop {
        let watchdog_race =
            embassy_futures::select::select(Timer::after_secs(5), sub.next_message_pure()).await;

        match watchdog_race {
            embassy_futures::select::Either::First(_) => {
                warn!("Button wasn't pressed for 5 seconds already!")
            }
            embassy_futures::select::Either::Second(_) => (),
        }
    }
}

#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>) {
    info!("Button task ready!");
    let publ = BUTTON_PRESSED.publisher().unwrap();
    let mut is_fast = false;

    loop {
        button.wait_for_rising_edge().await;
        info!("Pressed!");

        is_fast = !is_fast;
        let delay = if is_fast { 300 } else { 1000 };

        publ.publish_immediate(delay);

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

    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down, Irqs);
    spawner.spawn(button_task(button)).unwrap();
    spawner.spawn(logger_task()).unwrap();
    spawner.spawn(watchdog_task()).unwrap();

    let mut is_turned_on = false;
    let mut delay = 1000u32;
    let mut sub = BUTTON_PRESSED.subscriber().unwrap();

    loop {
        if is_turned_on {
            red_led.set_high();
            green_led.set_high();
        } else {
            red_led.set_low();
            green_led.set_low();
        }

        let loop_delay = Timer::after_millis(delay as u64);

        let signal_race =
            embassy_futures::select::select(loop_delay, sub.next_message_pure()).await;

        match signal_race {
            embassy_futures::select::Either::First(_) => (),
            embassy_futures::select::Either::Second(new_delay) => {
                delay = new_delay;
                continue;
            }
        }

        is_turned_on = !is_turned_on;

        {
            let mut counter = COUNTER.lock().await;
            *counter += 1;
        }
    }
}
