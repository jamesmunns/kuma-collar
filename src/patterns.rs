use crate::hal;
use embedded_hal::blocking::delay::DelayMs;
use smart_leds::{SmartLedsWrite, RGB, gamma};
use stm32f4xx_hal::delay::Delay;
use stm32f4xx_hal::gpio::{
    gpioa::{PA5, PA6, PA7},
    Alternate, AF5,
};
use stm32f4xx_hal::spi::Spi;
use stm32f4xx_hal::stm32::SPI1;
use ws2812_spi::Ws2812;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::RngCore;
use rtt_target::rprintln;
use rand::Rng;

type SmartLed = Ws2812<
    Spi<
        SPI1,
        (
            PA5<Alternate<AF5>>,
            PA6<Alternate<AF5>>,
            PA7<Alternate<AF5>>,
        ),
    >,
>;

pub fn run_all(
    smartled: &mut SmartLed,
    delay: &mut Delay,
    rng: &mut ChaCha8Rng,
) {
    // rprintln!("poc_pulse");
    // poc_pulse(smartled, delay, rng);
    rprintln!("smooth_pulse");
    smooth_pulse(smartled, delay, rng);
}

pub fn smooth_pulse(
    smartled: &mut SmartLed,
    delay: &mut Delay,
    rng: &mut ChaCha8Rng,
) {
    for i in 0..1 {
        rprintln!("Smooth iter {}", i);
        let red = rng.gen_range(0.0f32, 1.0f32);
        let green = rng.gen_range(0.0f32, 1.0f32);
        let blue = rng.gen_range(0.0f32, 1.0f32);
        rprintln!("r: {:1.02}, g: {:1.02}, b: {:1.02}", red, green, blue);

        for i in 0..1024 {
            let r = red * ((i as f32) / 1024.0) * 0x0F_FF as f32;
            let g = green * ((i as f32) / 1024.0) * 0x0F_FF as f32;
            let b = blue * ((i as f32) / 1024.0) * 0x0F_FF as f32;

            let y = RGB {
                r: linear_interp_gamma(r as u16),
                g: linear_interp_gamma(g as u16),
                b: linear_interp_gamma(b as u16),
            };
            let z = [y];
            let x = z.iter().cloned().cycle().take(30);
            write_oversample(smartled, x).ok();
        }
        for i in (0..1024).rev() {
            let r = red * ((i as f32) / 1024.0) * 0x0F_FF as f32;
            let g = green * ((i as f32) / 1024.0) * 0x0F_FF as f32;
            let b = blue * ((i as f32) / 1024.0) * 0x0F_FF as f32;

            let y = RGB {
                r: linear_interp_gamma(r as u16),
                g: linear_interp_gamma(g as u16),
                b: linear_interp_gamma(b as u16),
            };
            let z = [y];
            let x = z.iter().cloned().cycle().take(30);
            write_oversample(smartled, x).ok();
        }
    }
}

const GAMMA8: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4,
    4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11,
    12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22,
    22, 23, 24, 24, 25, 25, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37,
    38, 39, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58,
    59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85,
    86, 87, 89, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104, 105, 107, 109, 110, 112, 114,
    115, 117, 119, 120, 122, 124, 126, 127, 129, 131, 133, 135, 137, 138, 140, 142, 144,
    146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 167, 169, 171, 173, 175, 177, 180,
    182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213, 215, 218, 220,
    223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];

use libm::powf;

fn linear_interp_gamma(inp: u16) -> u16 {
    /*
        (int)(
            pow(
                (float)i / (float)max_in,
                gamma
            )
            * max_out + 0.5
        )
    */
    let base = (inp & 0x0F_FF) as f32;
    (powf(base / (0x0F_FF as f32), 2.8) * (0x0F_FF as f32) + 0.5) as u16
}


fn write_oversample<T, I>(led: &mut T, iter: I) -> Result<(), ()>
where
    T: SmartLedsWrite<Color = RGB<u8>>,
    I: Iterator<Item = RGB<u16>> + core::clone::Clone,
{
    for i in 0..16 {
        let iter2 = iter.clone();
        let miter = iter2.map(|rgb16| -> RGB<u8> {
            let corr_r = (rgb16.r & 0xF) as usize;
            let base_r = (rgb16.r >> 4) as u8;
            let corr_g = (rgb16.g & 0xF) as usize;
            let base_g = (rgb16.g >> 4) as u8;
            let corr_b = (rgb16.b & 0xF) as usize;
            let base_b = (rgb16.b >> 4) as u8;
            RGB {
                r: base_r + CORRECTIONS[corr_r][i],
                g: base_g + CORRECTIONS[corr_g][i],
                b: base_b + CORRECTIONS[corr_b][i],
            }
        });

        write(led, miter)?;
    }
    Ok(())
}

pub fn poc_pulse(
    smartled: &mut SmartLed,
    delay: &mut Delay,
    rng: &mut ChaCha8Rng,
) {
    for i in 0..1 {
        rprintln!("Chunky iter {}", i);
        let bytes = rng.next_u32().to_ne_bytes();
        let red = bytes[0];
        let green = bytes[1];
        let blue = bytes[2];

        for i in 0..(red.max(green).max(blue)) {
            let y = RGB {
                r: red.min(i),
                g: green.min(i),
                b: blue.min(i),
            };
            let z = [y];
            // On for 1s, off for 1s.
            let x = gamma(z.iter().cloned().cycle().take(30));
            write(smartled, x).ok();
            delay.delay_ms(16u32);
        }
        for i in (0..(red.max(green).max(blue))).rev() {
            let y = RGB {
                r: red.min(i),
                g: green.min(i),
                b: blue.min(i),
            };
            let z = [y];
            // On for 1s, off for 1s.
            let x = gamma(z.iter().cloned().cycle().take(30));
            write(smartled, x).ok();

            delay.delay_ms(16u32);
        }
    }
}


fn write<T, I>(led: &mut T, iter: I) -> Result<(), ()>
where
    T: SmartLedsWrite<Color = RGB<u8>>,
    I: Iterator<Item = RGB<u8>>,
{
    let res = led.write(iter);
    unsafe {
        let _ = (*hal::stm32::SPI1::ptr()).dr.read();
        let _ = (*hal::stm32::SPI1::ptr()).sr.read();
    }
    res.map_err(drop)
}

const CORRECTIONS: &[&[u8; 16]; 16] = &[
    &[0; 16],                                           // 0
    &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],  // 1
    &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],  // 2
    &[0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0],  // 3
    &[0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],  // 4
    &[0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0],  // 5
    &[0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0],  // 6
    &[0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1],  // 7
    &[0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],  // 8
    &[0, 1, 0, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1],  // 9
    &[0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 1],  // 10
    &[0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1],  // 11
    &[0, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1],  // 12
    &[0, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1],  // 13
    &[0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1],  // 14
    &[0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],  // 15
];
