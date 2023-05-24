//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_halt as _;

use nb::block;

use cortex_m_rt::entry;
use stm32f1xx_hal::{pac, prelude::*,serial::{self, Config, Serial, StopBits}};
use core::fmt::Write;

const MSG_MAX_LEN: usize = u8::MAX as usize;

fn receive_command<RX>(serial_rx: &mut RX, buf: &mut [u8; MSG_MAX_LEN]) -> usize
    where RX: embedded_hal::serial::Read<u8, Error = serial::Error>
{
    enum RxState {
        Length,
        Data { len: usize, idx: usize },
    }

    let mut rx_phase = RxState::Length;

    loop {
        // Read the word that was just sent. Blocks until the read is complete.
        let received = block!(serial_rx.read()).unwrap();

        match rx_phase {
            RxState::Length => {
                if received >= 48 && received <= 57 {
                    let cmd_length = received - 48;
                    if cmd_length == 0 {
                        return 0;
                    }
                    rx_phase = RxState::Data {
                        len: cmd_length as usize,
                        idx: 0,
                    };
                } else {
                    rx_phase = RxState::Length;
                }
            }

            RxState::Data { len, ref mut idx } => {
                buf[*idx] = received as u8;
                *idx += 1;
                if *idx == len {
                    return len;
                }
            }
        }
    }
}

#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let _cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and AFIO I/O registers and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    let mut gpioa = dp.GPIOA.split();

    let tx = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx = gpioa.pa3;
    let mut serial = Serial::new(
        dp.USART2,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(115200.bps()).wordlength_8bits().parity_none().stopbits(StopBits::STOP1),
        &clocks
    );

    writeln!(serial.tx, "Please type characters to echo:\r\n").unwrap();

    let mut buf = [0u8; MSG_MAX_LEN];

    /* Endless loop */
    loop {
        // Receive message from master device.
        let received_msg_len = receive_command(&mut serial.rx, &mut buf);
        for i in 0..received_msg_len {
            writeln!(serial.tx, "Inputted character is {}\r\n", buf[i]).unwrap();
        }
    }
}

// LED blinking
// #[entry]
// fn main() -> ! {
//     // Get access to the core peripherals from the cortex-m crate
//     let cp = cortex_m::Peripherals::take().unwrap();
//     // Get access to the device specific peripherals from the peripheral access crate
//     let dp = pac::Peripherals::take().unwrap();
//
//     // Take ownership over the raw flash and rcc devices and convert them into the corresponding
//     // HAL structs
//     let mut flash = dp.FLASH.constrain();
//     let rcc = dp.RCC.constrain();
//
//     // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
//     // `clocks`
//     let clocks = rcc.cfgr.freeze(&mut flash.acr);
//
//     // Acquire the GPIOA peripheral
//     let mut gpioa = dp.GPIOA.split();
//
//     // Configure gpio A pin 5 as a push-pull output. The `crh` register is passed to the function
//     // in order to configure the port. For pins 0-7, crl should be passed instead.
//     let mut led = gpioa.pa5.into_push_pull_output(&mut gpioa.crl);
//     // Configure the syst timer to trigger an update every second
//     let mut timer = Timer::syst(cp.SYST, &clocks).counter_hz();
//     timer.start(1.Hz()).unwrap();
//
//     // Wait for the timer to trigger an update and change the state of the LED
//     loop {
//         block!(timer.wait()).unwrap();
//         led.set_high();
//         block!(timer.wait()).unwrap();
//         led.set_low();
//     }
// }