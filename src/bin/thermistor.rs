#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;

use embassy_executor::Spawner;
use embassy_stm32::Config;
use embassy_stm32::adc::{Adc, SampleTime};
use embassy_stm32::peripherals::ADC1;
use embassy_stm32::{adc, bind_interrupts, peripherals};
use embassy_time::{Duration, Timer};
use fmt::{error, info};
use libm::logf;

#[cfg(not(feature = "defmt"))]
use panic_halt as _;
#[cfg(feature = "defmt")]
use {defmt_rtt as _, panic_probe as _};

// ── Thermistor constants ──────────────────────────────────────────────────────
const THERMISTOR_NOMINAL: f32 = 100_000.0;
const TEMP_NOMINAL: f32 = 25.0; // degrees C at nominal resistance
const B_COEFFICIENT: f32 = 3950.0; // B value — check your datasheet
const SERIES_RESISTOR: f32 = 100_000.0;
const ADC_MAX: f32 = 65535.0; // 16-bit

/// Convert raw ADC reading to degrees Celsius
/// using the Steinhart-Hart B-parameter equation
fn adc_to_celsius(raw: u16) -> f32 {
    // voltage divider: thermistor on top, R1 on bottom
    // Vout = 3.3V * R1 / (Rtherm + R1)
    // so Rtherm = R1 * (ADC_MAX / raw - 1)
    let raw_f = raw as f32;

    if raw_f <= 0.0 || raw_f >= ADC_MAX {
        return f32::NAN; // open or short circuit
    }

    let resistance = SERIES_RESISTOR * (ADC_MAX / raw_f - 1.0);

    // Steinhart-Hart B-parameter equation:
    // 1/T = 1/T0 + (1/B) * ln(R/R0)
    let t0_kelvin = TEMP_NOMINAL + 273.15;
    let steinhart = 1.0 / t0_kelvin + (1.0 / B_COEFFICIENT) * logf(resistance / THERMISTOR_NOMINAL);
    let temp_kelvin = 1.0 / steinhart;

    temp_kelvin - 273.15
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV8), // SPI1 cksel defaults to pll1_q
            divr: None,
        });
        config.rcc.pll2 = Some(Pll {
            source: PllSource::HSI,
            prediv: PllPreDiv::DIV4,
            mul: PllMul::MUL50,
            divp: Some(PllDiv::DIV8), // 100mhz
            divq: None,
            divr: None,
        });
        config.rcc.sys = Sysclk::PLL1_P; // 400 Mhz
        config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 Mhz
        config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb2_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb3_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.apb4_pre = APBPrescaler::DIV2; // 100 Mhz
        config.rcc.voltage_scale = VoltageScale::Scale1;
        config.rcc.mux.adcsel = mux::Adcsel::PLL2_P;
    }
    let mut p = embassy_stm32::init(config);

    info!("STM32H723ZG thermistor reader starting");

    let mut adc = Adc::new(p.ADC1);

    loop {
        let mut sum: u32 = 0;
        for _ in 0..10u32 {
            sum += adc.blocking_read(&mut p.PA3, SampleTime::CYCLES32_5) as u32;
        }

        let raw = (sum / 10) as u16;

        let temp = adc_to_celsius(raw);
        let resistance = SERIES_RESISTOR * (ADC_MAX / raw as f32 - 1.0);

        if temp.is_nan() {
            error!("ADC reading out of range — check wiring (raw={})", raw);
        } else {
            info!(
                "raw={:05}  resistance={} ohm  temp={} C",
                raw, resistance, temp
            );
        }

        Timer::after(Duration::from_millis(500)).await;
    }
}
