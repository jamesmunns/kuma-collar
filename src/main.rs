#![no_main]
#![no_std]

// Halt on panic
#[allow(unused_extern_crates)] // NOTE(allow) bug rust-lang/rust#53964
extern crate panic_reset; // panic handler

use crate::hal::{prelude::*, stm32};
use cortex_m;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32f4xx_hal as hal;
use ws2812_spi;
use hal::adc::{Adc, config::AdcConfig};

use rand_chacha::{
    ChaCha8Rng,
    rand_core::SeedableRng,
};

mod patterns;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    if let (Some(dp), Some(cp)) = (
        stm32::Peripherals::take(),
        cortex_m::peripheral::Peripherals::take(),
    ) {
        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(24.mhz())
            .require_pll48clk()
            .sysclk(96.mhz())
            .hclk(96.mhz())
            .pclk1(48.mhz())
            .pclk2(96.mhz())
            .freeze();

        // Set up the LED. On the Nucleo-446RE it's connected to pin PA5.
        let gpioa = dp.GPIOA.split();
        let mut led1 = gpioa.pa0.into_push_pull_output();

        let spi = hal::spi::Spi::spi1(
            dp.SPI1,
            (
                gpioa.pa5.into_alternate_af5(),
                gpioa.pa6.into_alternate_af5(),
                gpioa.pa7.into_alternate_af5(),
            ),
            hal::spi::Mode {
                polarity: hal::spi::Polarity::IdleLow,
                phase: hal::spi::Phase::CaptureOnFirstTransition,
            },
            3_000_000.hz(),
            clocks,
        );


        let mut smartled = ws2812_spi::Ws2812::new(spi);

        // Create a delay abstraction based on SysTick
        let mut delay = hal::delay::Delay::new(cp.SYST, clocks);

        let cfg = AdcConfig::default();

        use hal::adc::config;
        let mut adc = Adc::adc1(dp.ADC1, true, cfg);
        adc.enable_temperature_and_vref();
        adc.set_resolution(config::Resolution::Twelve);

        let mut last_key = 0;

        let mut keys = [0u32; 8];

        let mut cum_samples: usize = 0;

        for idx in 0..8 {
            let mut key: u32 = 0xACACACAC;
            let mut last: u16 = 1000;
            let mut mix_last: u16 = 1000;
            let mut mix: u32 = 0xABCD_EF12;
            let mut run: u32 = 0;
            let mut ops: u32 = 0;

            rprintln!("Gathering entropy");

            // TODO: Count samples a

            while ops < 1000 {
                let sample = adc.convert(&hal::adc::Temperature, config::SampleTime::Cycles_480);
                cum_samples = cum_samples.checked_add(1).unwrap();

                if sample == last {
                    run += 1;
                    continue;
                }

                last = sample;

                if run != 0 && run.count_ones() != 1 {
                    key = key.wrapping_mul(run);
                    ops += 1;
                }

                run = 0;

                let candidate_bits = (mix_last ^ sample) & 0b11;
                if candidate_bits != 0 {
                    mix <<= 2;
                    mix |= (sample & 0b11) as u32;
                    mix_last = sample;
                    key = mix.wrapping_mul(mix);
                    ops += 1;
                } else {
                    key = key.wrapping_shl(7);
                }
            }
            keys[idx] = key;
            rprintln!("Seed: 0x{:08X}", key);
            rprintln!("entropy: {}", (last_key ^ key).count_ones());
            rprintln!("cum samples: {}", cum_samples);
            last_key = key;
        }

        let mut key = [0u8; 32];
        let mut idx = 0;

        for i in keys.iter() {
            for j in i.to_ne_bytes().iter() {
                key[idx] = *j;
                idx += 1;
            }
        }
        rprintln!("Final Seed: {:02X?}", key);

        let mut rng = ChaCha8Rng::from_seed(key);

        rprintln!("START");
        led1.set_high().ok();
        delay.delay_ms(3000u32);

        loop {
            patterns::run_all(&mut smartled, &mut delay, &mut rng);
        }
    }

    loop {}
}
