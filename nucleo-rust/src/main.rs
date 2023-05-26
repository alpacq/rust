//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

//#![deny(unsafe_code)]
#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::{pac::{self, interrupt, USART2}, prelude::*, serial::{Config, Serial, StopBits, Tx, Rx}};
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
            to.args.push(self.args[i]).unwrap();
        }
    }
}

static mut RX: Option<Rx<USART2>> = None;
static mut TX: Option<Tx<USART2>> = None;
static mut CURRENT_COMMAND: Command = Command { len: 1, cmd: 0, args: Vec::new() };
static mut RX_STATE: RxState = RxState::Length;

unsafe fn uart_command_response(command: &Command) {
    if let Some(tx) = TX.as_mut() {
        writeln!(tx, "Length of cmd is {}\r", command.len).unwrap();
        writeln!(tx, "Command code is {}\r", command.cmd).unwrap();
        for i in 0..command.args.len() {
            writeln!(tx, "Argument {} is {}\r", i, command.args[i]).unwrap();
        }
    }
}

#[interrupt]
unsafe fn USART2() {
    cortex_m::interrupt::free(|_| {
        if let Some(rx) = RX.as_mut() {
            while rx.is_rx_not_empty() {
                if let Ok(received) = nb::block!(rx.read()) {
                    match RX_STATE {
                        RxState::Length => {
                            if received >= 48 && received <= 57 {
                                let cmd_length = received - 48;
                                if cmd_length == 0 {
                                    return;
                                }

                                RX_STATE = RxState::Data {
                                    command: Command::new(cmd_length as usize),
                                    idx: 0,
                                };
                            } else {
                                RX_STATE = RxState::Length;
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
                                command.copy(&mut CURRENT_COMMAND);
                                RX_STATE = RxState::Length;
                                uart_command_response(&CURRENT_COMMAND);
                            }
                        }
                    }
                }
                rx.listen_idle();
            }
            if rx.is_idle() {
                rx.unlisten_idle();
            }
        }
    })
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

    serial.tx.listen();
    serial.rx.listen();
    serial.rx.listen_idle();

    writeln!(serial.tx, "Please type command |len||cmd||args..|:\r\n").unwrap();

    cortex_m::interrupt::free(|_| unsafe {
        TX.replace(serial.tx);
        RX.replace(serial.rx);
    });
    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART2);
    }

    loop {
        cortex_m::asm::wfi()
    }
}