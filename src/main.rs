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
    setup_eth(&soc_periph);

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

fn setup_eth(periph: &stm32f407::Peripherals) {
    // We'll need pins in these banks.
    periph.RCC.ahb1enr.modify(|_r, w| w
        .gpioaen().enabled()
        .gpioben().enabled()
        .gpiocen().enabled()
        .gpioeen().enabled());

    // The PHY achieves a low pin count by multiplexing important configuration
    // inputs onto its pins.  These pins are sampled when the PHY is reset,
    // the pins are changed to their normal function, and the configuration is
    // used until the PHY is reset again.
    // Some of this PHY configuration can be set using the SMI after the PHY
    // has booted, but some of it cannot.  Luckily, the ones that cannot are
    // configured by pins that are permanently tied to useful values on the PCB.
    // Because of this we can just set up our pins for the MAC (plus the PHY
    // reset pin) and leave them.

    // Ensure the PHY is not in reset. PE2, active low.
    periph.GPIOE.moder.modify(|_r, w| w.moder2().output());
    periph.GPIOE.otyper.modify(|_r, w| w.ot2().push_pull());
    periph.GPIOE.bsrr.write(|w| w.bs2().set());

    // Hold the MAC in reset while we configure it to operate in RMII mode.
    periph.RCC.ahb1rstr.modify(|_r, w| w.ethmacrst().reset());
    periph.RCC.ahb1enr.modify(|_r, w| w
        .ethmacptpen().disabled()
        .ethmacrxen().disabled()
        .ethmactxen().disabled()
        .ethmacen().disabled());
    periph.SYSCFG.pmc.modify(|_r, w| w.mii_rmii_sel().set_bit());

    // Set up the pins as required for the following RMII and SMI signals:
    // ETH_RMII_REF_CLK -- PA1
    // ETH_MDIO -- PA2
    // ETH_RMII_CRS_DV -- PA7
    // ETH_RMII_TX_EN -- PB11
    // ETH_RMII_TXD[1:0] -- PB13 and PB12
    // ETH_MDC -- PC1
    // ETH_RMII_RXD[1:0] -- PC5 and PC4
    //
    // PA1, 2, 7
    periph.GPIOA.afrl.modify(|_r, w| w
        .afrl1().af11()
        .afrl2().af11()
        .afrl7().af11());
    periph.GPIOA.moder.modify(|_r, w| w
        .moder1().alternate()
        .moder2().alternate()
        .moder7().alternate());
    periph.GPIOA.pupdr.modify(|_r, w| w
        .pupdr1().floating()
        .pupdr2().floating()  // MDIO has external pullup.
        .pupdr7().floating());
    periph.GPIOA.otyper.modify(|_r, w| w
        .ot2().open_drain());
    periph.GPIOA.ospeedr.modify(|_r, w| w
        .ospeedr2().low_speed());
    // PB11, 12, 13
    periph.GPIOB.afrh.modify(|_r, w| w
        .afrh11().af11()
        .afrh12().af11()
        .afrh13().af11());
    periph.GPIOB.moder.modify(|_r, w| w
        .moder11().alternate()
        .moder12().alternate()
        .moder13().alternate());
    periph.GPIOB.pupdr.modify(|_r, w| w
        .pupdr11().floating()
        .pupdr12().floating()
        .pupdr13().floating());
    periph.GPIOB.otyper.modify(|_r, w| w
        .ot11().push_pull()
        .ot12().push_pull()
        .ot13().push_pull());
    periph.GPIOB.ospeedr.modify(|_r, w| w
        .ospeedr11().high_speed()
        .ospeedr12().high_speed()
        .ospeedr13().high_speed());
    // PC1, 4, 5
    periph.GPIOC.afrl.modify(|_r, w| w
        .afrl1().af11()
        .afrl4().af11()
        .afrl5().af11());
    periph.GPIOC.moder.modify(|_r, w| w
        .moder1().alternate()
        .moder4().alternate()
        .moder5().alternate());
    periph.GPIOC.pupdr.modify(|_r, w| w
        .pupdr1().floating()
        .pupdr4().floating()
        .pupdr5().floating());
    periph.GPIOC.otyper.modify(|_r, w| w
        .ot1().push_pull());
    periph.GPIOC.ospeedr.modify(|_r, w| w
        .ospeedr1().low_speed());

    // If board capacitance is high we might need to boost some output speeds
    // to very high speed (11).  We might then need to enable the IO
    // compensation cell.

    // Take the MAC out of reset.
    periph.RCC.ahb1enr.modify(|_r, w| w
        .ethmacrxen().enabled()
        .ethmactxen().enabled()
        .ethmacen().enabled());
    periph.RCC.ahb1rstr.modify(|_r, w| w.ethmacrst().clear_bit());

    // Set up SMI comms between the MAC and PHY.
    // PHY is hard coded to use address 0.
    while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
    periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
        .pa().bits(0)
        .cr().cr_150_168());

    // Read some PHY identifying information (OUI and model number).
    // These are known values, so use these to check that comms are working
    // properly.
    periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
        .mr().bits(2)
        .mw().read()
        .mb().busy());
    while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
    let phy_id_1 = periph.ETHERNET_MAC.macmiidr.read().td().bits();
    if phy_id_1 != 0x07 {
        panic!("bad read of phy r2");
    }
    periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
        .mr().bits(3)
        .mw().read()
        .mb().busy());
    while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
    let phy_id_2 = periph.ETHERNET_MAC.macmiidr.read().td().bits();
    if (phy_id_2 & 0xFFF0) != 0xC0F0 {
        panic!("bad read of phy r3");
    }

    // Debug.  What does the status register look like at this point?
    periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
        .mr().bits(0)
        .mw().read()
        .mb().busy());
    while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
    let status = periph.ETHERNET_MAC.macmiidr.read().td().bits();
    let mut hstdout = hio::hstdout().unwrap();
    writeln!(hstdout, "DEBUG status: {}", status).unwrap();

    // Override the initial configuration caused by the PHY's MODE pins.
    periph.ETHERNET_MAC.macmiidr.write(|w| w.td().bits(
        (1<<14) |  // reserved value
        (0b111 << 5)));  // autonegotiate any supported
    // Soft reset the PHY so that the MODE takes effect.
    periph.ETHERNET_MAC.macmiidr.write(|w| w.td().bits(1<<15));
    periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
        .mr().bits(0)
        .mw().write()
        .mb().busy());
    // Wait until the reset bit has cleared itself.
    loop {
        while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
        periph.ETHERNET_MAC.macmiiar.modify(|_r, w| w
            .mr().bits(0)
            .mw().read()
            .mb().busy());
        while periph.ETHERNET_MAC.macmiiar.read().mb().is_busy() {}
        let status = periph.ETHERNET_MAC.macmiidr.read().td().bits();
        if (status & 0x8000) == 0 {
            writeln!(hstdout, "DEBUG status 2: {}", status).unwrap();
            break;
        }
    }
}
