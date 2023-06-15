//! Blinks an LED
//!
//! This assumes that a LED is connected to pc13 as is the case on the blue pill board.
//!
//! Note: Without additional hardware, PC13 should not be used to drive an LED, see page 5.1.2 of
//! the reference manual for an explanation. This is not an issue on the blue pill.

//#![deny(unsafe_code)]
#![no_std]
#![no_main]

mod command;

use panic_halt as _;
use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac::{self, interrupt, USART2, SPI2},
    prelude::*,
    gpio::{Pin, Output, Alternate},
    spi::{self, Spi, Spi2NoRemap},
    serial::{Config, Serial, StopBits, Tx, Rx}};
use core::fmt::Write;
use heapless::Vec;
use command::{Command, RxState};
use lcd_hal::{Display, pcd8544::spi::Pcd8544Spi};

static mut RX: Option<Rx<USART2>> = None;
static mut TX: Option<Tx<USART2>> = None;
static mut CURRENT_COMMAND: Command = Command { len: 1, cmd: 0, args: Vec::new() };
static mut RX_STATE: RxState = RxState::Length;
static mut DISPLAY: Option<Pcd8544Spi<Spi<SPI2, Spi2NoRemap, (Pin<'B', 13, Alternate>, Pin<'B', 14>, Pin<'B', 15, Alternate>), u8>, Pin<'C', 7, Output>, Pin<'B', 10, Output>>> = None;
static mut LIGHT: Option<Pin<'A', 10, Output>> = None;

unsafe fn uart_command_response() {
    if let Some(tx) = TX.as_mut() {
        writeln!(tx, "Length of cmd is {}\r", CURRENT_COMMAND.len).unwrap();
        writeln!(tx, "Command code is {}\r", CURRENT_COMMAND.cmd).unwrap();
        for i in 0..CURRENT_COMMAND.args.len() {
            writeln!(tx, "Argument {} is {}\r", i, CURRENT_COMMAND.args[i]).unwrap();
        }
    }
}

unsafe fn execute_command() {
    match CURRENT_COMMAND.cmd {
        107 => { //K
            if let Some(display) = DISPLAY.as_mut() {
                display.clear().unwrap();
                let res = display.print(b"Hello Kris");
                if let Some(tx) = TX.as_mut() {
                    match res {
                        Ok(_) => writeln!(tx, "Write performed\r\n").unwrap(),
                        Err(_) => writeln!(tx, "Write failed\r\n").unwrap()
                    };
                }
            }
        }
        108 => { //L
            if let Some(light) = LIGHT.as_mut() {
                light.set_high();
            }
        }
        115 => { //S
            if let Some(light) = LIGHT.as_mut() {
                light.set_low();
            }
        }
        _ => {}
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
                                execute_command();
                                uart_command_response();
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
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .freeze(&mut flash.acr);

    let mut gpioa = dp.GPIOA.split();
    let mut gpiob = dp.GPIOB.split();
    let mut gpioc = dp.GPIOC.split();

    let sck = gpiob.pb13.into_alternate_push_pull(&mut gpiob.crh);
    let mosi = gpiob.pb15.into_alternate_push_pull(&mut gpiob.crh);
    let miso = gpiob.pb14.into_floating_input(&mut gpiob.crh);
    let cs = gpiob.pb10.into_push_pull_output(&mut gpiob.crh);

    let spi_mode = spi::Mode {
        phase: spi::Phase::CaptureOnFirstTransition,
        polarity: spi::Polarity::IdleLow,
    };
    let spi = Spi::spi2(
        dp.SPI2,
        (sck, miso, mosi),
        spi_mode,
        4.MHz(),
        clocks
    );

    let mut bl = gpioa.pa10.into_push_pull_output(&mut gpioa.crh);
    let dc = gpioc.pc7.into_push_pull_output(&mut gpioc.crl);
    let mut rst = gpioa.pa8.into_push_pull_output(&mut gpioa.crh);

    let mut delay = cp.SYST.delay(&clocks);

    bl.set_high();

    let tx_pin = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx_pin = gpioa.pa3;
    let mut serial = Serial::new(
        dp.USART2,
        (tx_pin, rx_pin),
        &mut afio.mapr,
        Config::default().baudrate(115200.bps()).wordlength_8bits().parity_none().stopbits(StopBits::STOP1),
        &clocks
    );

    serial.tx.listen();
    serial.rx.listen();
    serial.rx.listen_idle();

    let mut display = Pcd8544Spi::new(spi, dc, cs, &mut rst, &mut delay).unwrap();

    let res = display.print(b"Hello world");
    match res {
        Ok(_) => writeln!(serial.tx, "Write performed\r\n").unwrap(),
        Err(_) => writeln!(serial.tx, "Write failed\r\n").unwrap()
    };

    writeln!(serial.tx, "Please type command |len||cmd||args..|:\r\n").unwrap();

    cortex_m::interrupt::free(|_| unsafe {
        TX.replace(serial.tx);
        RX.replace(serial.rx);
        DISPLAY.replace(display);
        LIGHT.replace(bl);
    });
    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART2);
    }

    loop {
        cortex_m::asm::wfi()
    }
}