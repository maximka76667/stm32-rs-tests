#![no_std]

use embedded_hal::i2c::I2c;

const ADDRESS: u8 = 0x68;
const PWR_MGMT_1: u8 = 0x6B;
const ACCEL_XOUT_H: u8 = 0x3B;
const GYRO_XOUT_H: u8 = 0x43;
const WHOAMI_REG: u8 = 0x75;
const WHOAMI_VAL: u8 = 0x70;

pub struct Mpu6500<I2C> {
    i2c: I2C,
    accel_offset: (i32, i32, i32),
    gyro_offset: (i32, i32, i32),
}

#[derive(Debug)]
pub enum Error<E> {
    I2c(E),
    InvalidDevice, // whoami returned wrong value
}

impl<I2C: I2c> Mpu6500<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            accel_offset: (0, 0, 0),
            gyro_offset: (0, 0, 0),
        }
    }

    pub fn init(&mut self) -> Result<(), Error<I2C::Error>> {
        // check whoami
        let mut buf = [0u8; 1];
        self.i2c
            .write_read(ADDRESS, &[WHOAMI_REG], &mut buf)
            .map_err(Error::I2c)?;

        if buf[0] != WHOAMI_VAL {
            return Err(Error::InvalidDevice);
        }

        // wake up
        self.i2c
            .write(ADDRESS, &[PWR_MGMT_1, 0x00])
            .map_err(Error::I2c)?;

        Ok(())
    }

    pub fn calibrate(&mut self, samples: i32) -> Result<(), Error<I2C::Error>> {
        let mut accel_x_sum: i32 = 0;
        let mut accel_y_sum: i32 = 0;
        let mut accel_z_sum: i32 = 0;
        let mut gyro_x_sum: i32 = 0;
        let mut gyro_y_sum: i32 = 0;
        let mut gyro_z_sum: i32 = 0;

        for _ in 0..samples {
            let (ax, ay, az) = self.read_accel_raw()?;
            let (gx, gy, gz) = self.read_gyro_raw()?;

            accel_x_sum += ax as i32;
            accel_y_sum += ay as i32;
            accel_z_sum += az as i32;
            gyro_x_sum += gx as i32;
            gyro_y_sum += gy as i32;
            gyro_z_sum += gz as i32;
        }

        self.accel_offset = (
            accel_x_sum / samples,
            accel_y_sum / samples,
            accel_z_sum / samples + 16384, // remove gravity (upside down)
        );
        self.gyro_offset = (
            gyro_x_sum / samples,
            gyro_y_sum / samples,
            gyro_z_sum / samples,
        );

        Ok(())
    }

    fn read_accel_raw(&mut self) -> Result<(i16, i16, i16), Error<I2C::Error>> {
        let mut buf = [0u8; 6];
        self.i2c
            .write_read(ADDRESS, &[ACCEL_XOUT_H], &mut buf)
            .map_err(Error::I2c)?;
        Ok((
            i16::from_be_bytes([buf[0], buf[1]]),
            i16::from_be_bytes([buf[2], buf[3]]),
            i16::from_be_bytes([buf[4], buf[5]]),
        ))
    }

    fn read_gyro_raw(&mut self) -> Result<(i16, i16, i16), Error<I2C::Error>> {
        let mut buf = [0u8; 6];
        self.i2c
            .write_read(ADDRESS, &[GYRO_XOUT_H], &mut buf)
            .map_err(Error::I2c)?;
        Ok((
            i16::from_be_bytes([buf[0], buf[1]]),
            i16::from_be_bytes([buf[2], buf[3]]),
            i16::from_be_bytes([buf[4], buf[5]]),
        ))
    }

    /// Returns acceleration in g
    pub fn read_accel(&mut self) -> Result<(f32, f32, f32), Error<I2C::Error>> {
        let (x, y, z) = self.read_accel_raw()?;
        let (ox, oy, oz) = self.accel_offset;
        Ok((
            (x as i32 - ox) as f32 / 16384.0,
            (y as i32 - oy) as f32 / 16384.0,
            (z as i32 - oz) as f32 / 16384.0,
        ))
    }

    /// Returns angular velocity in degrees per second
    pub fn read_gyro(&mut self) -> Result<(f32, f32, f32), Error<I2C::Error>> {
        let (x, y, z) = self.read_gyro_raw()?;
        let (ox, oy, oz) = self.gyro_offset;
        Ok((
            (x as i32 - ox) as f32 / 131.0,
            (y as i32 - oy) as f32 / 131.0,
            (z as i32 - oz) as f32 / 131.0,
        ))
    }
}
