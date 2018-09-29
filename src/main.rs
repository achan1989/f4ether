#![no_std]
#![no_main]

// pick a panicking behavior
// extern crate panic_halt; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// extern crate panic_abort; // requires nightly
// extern crate panic_itm; // logs messages over ITM; requires ITM support
extern crate panic_semihosting; // logs messages to the host stderr; requires a debugger

extern crate cortex_m_semihosting;

use cortex_m_rt::{entry, exception};
use cortex_m_semihosting::hio;
use core::fmt::Write;

// TODO: use a standard device crate.
// This is currently a custom generate & build of the stm32f407 device crate
// from stm32-rs. The custom build allows it to be used with the beta toolchain.
// Would like to move back to using the standard crate once it builds in beta.
extern crate stm32f407;


#[entry]
fn main() -> ! {
    let soc_periph = stm32f407::Peripherals::take().unwrap();

    setup_clocks(&soc_periph);

    // Quick sanity check of the device crate -- light the blue LED.
    soc_periph.RCC.ahb1enr.modify(|_r, w| w.gpioden().enabled());
    soc_periph.GPIOD.moder.modify(|_r, w| w.moder15().output());
    soc_periph.GPIOD.otyper.modify(|_r, w| w.ot15().push_pull());
    soc_periph.GPIOD.bsrr.write(|w| w.bs15().set());

    loop {
        // your code goes here
    }
}

fn setup_clocks(periph: &stm32f407::Peripherals) {
    // Enable the external 8MHz oscillator and the clock security system.
    periph.RCC.cr.modify(|_r, w| w
        .csson().set_bit()
        .hseon().set_bit());
    // Wait for the external oscillator to stabilise.
    while periph.RCC.cr.read().hserdy().bit_is_clear() {}

    // Set the appropriate flash memory latency for operating at high speed,
    // enable cache and prefetch.
    periph.FLASH.acr.modify(|_r, w| w
        .latency().ws5()
        .prften().enabled()
        .icen().enabled()
        .dcen().enabled());

    // Set up various bus speeds divided down from the 168 MHz sysclk.
    // 168 / 1 = 168 MHz AHBCLK
    // 168 / 2 = 84 MHz APB2CLK
    // 168 / 4 = 42 MHz APB1CLK
    periph.RCC.cfgr.modify(|_r, w| w
        .hpre().div1()
        .ppre2().div2()
        .ppre1().div4());
    // Set up the PLL to take the 8 MHz external osc and output a 168 MHz
    // system clock.  Also generate a misc clock of 48 MHz.
    // VCOC = 8 * (N=336 / M=8) = 336 MHz
    // SYSC = VCOC / (P=2) = 168 MHz
    // MISCC = VCOC / (Q=7) = 48 MHz
    periph.RCC.pllcfgr.modify(|_r, w| unsafe { w
        .pllsrc().hse()
        .pllm().bits(8)
        .plln().bits(336)
        .pllp().div2()
        .pllq().bits(7)
    });

    // Enable the PLL and wait for it to stabilise.
    periph.RCC.cr.modify(|_r, w| w.pllon().set_bit());
    while periph.RCC.cr.read().pllrdy().bit_is_clear() {}

    // Select the PLL output as the sysclk.
    periph.RCC.cfgr.modify(|_r, w| w.sw().pll());
    if !periph.RCC.cfgr.read().sws().is_pll() {
        panic!("pll select failed");
    }
}
