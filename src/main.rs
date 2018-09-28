#![no_std]
#![no_main]

// pick a panicking behavior
extern crate panic_halt; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// extern crate panic_abort; // requires nightly
// extern crate panic_itm; // logs messages over ITM; requires ITM support
// extern crate panic_semihosting; // logs messages to the host stderr; requires a debugger

use cortex_m_rt::entry;

// TODO: use a standard device crate.
// This is currently a custom generate & build of the stm32f407 device crate
// from stm32-rs. The custom build allows it to be used with the beta toolchain.
// Would like to move back to using the standard crate once it builds in beta.
extern crate stm32f407;


#[entry]
fn main() -> ! {
    let soc_periph = stm32f407::Peripherals::take().unwrap();

    // Quick sanity check of the device crate -- light the blue LED.
    soc_periph.RCC.ahb1enr.modify(|_r, w| w.gpioden().enabled());
    soc_periph.GPIOD.moder.modify(|_r, w| w.moder15().output());
    soc_periph.GPIOD.otyper.modify(|_r, w| w.ot15().push_pull());
    soc_periph.GPIOD.bsrr.write(|w| w.bs15().set());

    loop {
        // your code goes here
    }
}
