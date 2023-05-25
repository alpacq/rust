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
use stm32f1xx_hal::{pac::{self}, prelude::*, serial::{self, Config, Serial, StopBits}};
use core::fmt::Write;
use heapless::Vec;

struct Command {
    cmd: u8,
    args: Vec<u8, 4>,
    len: usize
}

enum RxState {
    Length,
    Data { command: Command, idx: usize },
}

impl Command {
    fn new(length: usize) -> Command {
        Command {
            len: length,
            cmd: 0u8,
            args: Vec::new()
        }
    }

    fn copy(&self, to: &mut Command) {
        to.len = self.len;
        to.cmd = self.cmd;
        to.args = Vec::new();
        for i in 0..self.args.len() {
            to.args[i] = self.args[i];
        }
    }
}

// static mut RX: Option<Rx<pac::USART2>> = None;
// static mut TX: Option<Tx<pac::USART2>> = None;

fn receive_command<RX>(serial_rx: &mut RX, cmd: &mut Command) -> usize
    where RX: embedded_hal::serial::Read<u8, Error = serial::Error>
{
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
                        command: Command::new(cmd_length as usize),
                        idx: 0,
                    };
                } else {
                    rx_phase = RxState::Length;
                }
            }

            RxState::Data { ref mut command, ref mut idx } => {
                if *idx == 0 {
                    command.cmd = received as u8;
                } else {
                    command.args.push(received as u8).unwrap();
                }
                *idx += 1;
                if *idx == command.len {
                    command.copy(cmd);
                    return command.len;
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

    let mut current_command = Command::new(1);

    /* Endless loop */
    loop {
        // Receive message from master device.
        let received_msg_len = receive_command(&mut serial.rx, &mut current_command);
        for i in 0..received_msg_len - 1 {
            writeln!(serial.tx, "Inputted character is {}\r\n", current_command.args[i]).unwrap();
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