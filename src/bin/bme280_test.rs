#![no_std]
#![no_main]

#[path = "../fmt.rs"]
mod fmt;
use embassy_time::Timer;
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

const ADDRESS: u8 = 0x76;
const CHIP_ID_REG: u8 = 0xD0;
const CHIP_ID: u8 = 0x60;
const RESET_REG: u8 = 0xE0;
const CTRL_HUM_REG: u8 = 0xF2;
const CTRL_MEAS_REG: u8 = 0xF4;
const DATA_REG: u8 = 0xF7;
const CALIB_00_REG: u8 = 0x88;
const CALIB_26_REG: u8 = 0xE1;

bind_interrupts!(struct Irqs {
    I2C2_EV => i2c::EventInterruptHandler<peripherals::I2C2>;
    I2C2_ER => i2c::ErrorInterruptHandler<peripherals::I2C2>;
    DMA1_STREAM4 => dma::InterruptHandler<peripherals::DMA1_CH4>;
    DMA1_STREAM5 => dma::InterruptHandler<peripherals::DMA1_CH5>;
});

struct CalibData {
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,
    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
    dig_h1: u8,
    dig_h2: i16,
    dig_h3: u8,
    dig_h4: i16,
    dig_h5: i16,
    dig_h6: i8,
}

fn read_calib(i2c: &mut I2c<'_, embassy_stm32::mode::Async, i2c::Master>) -> CalibData {
    let mut c00 = [0u8; 26];
    let mut c26 = [0u8; 7];
    i2c.blocking_write_read(ADDRESS, &[CALIB_00_REG], &mut c00)
        .unwrap();
    i2c.blocking_write_read(ADDRESS, &[CALIB_26_REG], &mut c26)
        .unwrap();

    CalibData {
        dig_t1: u16::from_le_bytes([c00[0], c00[1]]),
        dig_t2: i16::from_le_bytes([c00[2], c00[3]]),
        dig_t3: i16::from_le_bytes([c00[4], c00[5]]),
        dig_p1: u16::from_le_bytes([c00[6], c00[7]]),
        dig_p2: i16::from_le_bytes([c00[8], c00[9]]),
        dig_p3: i16::from_le_bytes([c00[10], c00[11]]),
        dig_p4: i16::from_le_bytes([c00[12], c00[13]]),
        dig_p5: i16::from_le_bytes([c00[14], c00[15]]),
        dig_p6: i16::from_le_bytes([c00[16], c00[17]]),
        dig_p7: i16::from_le_bytes([c00[18], c00[19]]),
        dig_p8: i16::from_le_bytes([c00[20], c00[21]]),
        dig_p9: i16::from_le_bytes([c00[22], c00[23]]),
        dig_h1: c00[25],
        dig_h2: i16::from_le_bytes([c26[0], c26[1]]),
        dig_h3: c26[2],
        dig_h4: ((c26[3] as i16) << 4) | ((c26[4] as i16) & 0x0F),
        dig_h5: ((c26[5] as i16) << 4) | ((c26[4] as i16) >> 4),
        dig_h6: c26[6] as i8,
    }
}

fn compensate_temp(raw: i32, c: &CalibData) -> (i32, i32) {
    let var1 = ((raw >> 3) - ((c.dig_t1 as i32) << 1)) * (c.dig_t2 as i32) >> 11;
    let var2 = (((raw >> 4) - (c.dig_t1 as i32)) * ((raw >> 4) - (c.dig_t1 as i32)) >> 12)
        * (c.dig_t3 as i32)
        >> 14;
    let t_fine = var1 + var2;
    ((t_fine * 5 + 128) >> 8, t_fine)
}

fn compensate_pressure(raw: i32, t_fine: i32, c: &CalibData) -> u32 {
    let mut var1 = (t_fine as i64) - 128000;
    let mut var2 = var1 * var1 * (c.dig_p6 as i64);
    var2 += (var1 * (c.dig_p5 as i64)) << 17;
    var2 += (c.dig_p4 as i64) << 35;
    var1 = (var1 * var1 * (c.dig_p3 as i64) >> 8) + ((var1 * (c.dig_p2 as i64)) << 12);
    var1 = ((1i64 << 47) + var1) * (c.dig_p1 as i64) >> 33;
    if var1 == 0 {
        return 0;
    }
    let mut p = 1048576i64 - raw as i64;
    p = ((p << 31) - var2) * 3125 / var1;
    var1 = (c.dig_p9 as i64) * (p >> 13) * (p >> 13) >> 25;
    var2 = (c.dig_p8 as i64) * p >> 19;
    p = ((p + var1 + var2) >> 8) + ((c.dig_p7 as i64) << 4);
    p as u32
}

fn compensate_humidity(raw: i32, t_fine: i32, c: &CalibData) -> u32 {
    let mut x: i32 = t_fine - 76800;
    x = (((raw << 14)
        .wrapping_sub((c.dig_h4 as i32) << 20)
        .wrapping_sub((c.dig_h5 as i32).wrapping_mul(x)))
    .wrapping_add(16384))
        >> 15;
    x = x.wrapping_mul(
        ((((((x.wrapping_mul(c.dig_h6 as i32)) >> 10)
            .wrapping_mul(((x.wrapping_mul(c.dig_h3 as i32)) >> 11).wrapping_add(32768)))
            >> 10)
            .wrapping_add(2097152))
        .wrapping_mul(c.dig_h2 as i32)
        .wrapping_add(8192))
            >> 14,
    );
    x = x.wrapping_sub(
        ((((x >> 15).wrapping_mul(x >> 15)) >> 7).wrapping_mul(c.dig_h1 as i32)) >> 4,
    );
    x = x.clamp(0, 419430400);
    (x >> 12) as u32
}

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

    // Sanity check
    let mut buf = [0u8; 1];
    i2c.blocking_write_read(ADDRESS, &[CHIP_ID_REG], &mut buf)
        .unwrap();
    info!("Chip ID: {:#x} (expected 0x60)", buf[0]);

    // Soft reset
    i2c.blocking_write(ADDRESS, &[RESET_REG, 0xB6]).unwrap();

    // Wait for NVM to finish loading (bit 0 of status register 0xF3)
    loop {
        let mut status = [0u8; 1];
        i2c.blocking_write_read(ADDRESS, &[0xF3], &mut status)
            .unwrap();
        if status[0] & 0x01 == 0 {
            break; // im_update bit cleared, NVM copy done
        }
    }
    // Read calibration
    let calib = read_calib(&mut i2c);

    info!(
        "T1={} T2={} T3={}",
        calib.dig_t1, calib.dig_t2, calib.dig_t3
    );
    info!(
        "P1={} P2={} P3={}",
        calib.dig_p1, calib.dig_p2, calib.dig_p3
    );
    info!(
        "H1={} H2={} H3={}",
        calib.dig_h1, calib.dig_h2, calib.dig_h3
    );

    // Configure
    i2c.blocking_write(ADDRESS, &[CTRL_HUM_REG, 0b001]).unwrap();
    i2c.blocking_write(ADDRESS, &[CTRL_MEAS_REG, 0b_010_010_11])
        .unwrap();

    loop {
        let mut raw = [0u8; 8];
        i2c.blocking_write_read(ADDRESS, &[DATA_REG], &mut raw)
            .unwrap();

        let raw_press = ((raw[0] as i32) << 12) | ((raw[1] as i32) << 4) | ((raw[2] as i32) >> 4);
        let raw_temp = ((raw[3] as i32) << 12) | ((raw[4] as i32) << 4) | ((raw[5] as i32) >> 4);
        let raw_hum = ((raw[6] as i32) << 8) | (raw[7] as i32);

        let (temp, t_fine) = compensate_temp(raw_temp, &calib);
        let press = compensate_pressure(raw_press, t_fine, &calib);
        let hum = compensate_humidity(raw_hum, t_fine, &calib);

        info!(
            "Temp: {}.{} C | Pressure: {} Pa | Humidity: {}.{} %",
            temp / 100,
            temp % 100,
            press / 256,
            hum / 1024,
            (hum % 1024) * 100 / 1024,
        );

        Timer::after_millis(500).await;
    }
}
