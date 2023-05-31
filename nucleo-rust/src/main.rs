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
    pac::{self, interrupt, USART2},
    prelude::*,
    serial::{Config, Serial, StopBits, Tx, Rx},
    i2c::{BlockingI2c, DutyCycle, Mode}, timer::Timer};
use nb::block;
use core::fmt::Write;
use heapless::Vec;
use command::{Command, RxState};
use mcp9808::{MCP9808, reg_res::ResolutionVal, reg_temp_generic::ReadableTempRegister};

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

unsafe fn uart_send_temp(temp: f32) {
    if let Some(tx) = TX.as_mut() {
        writeln!(tx, "Temperature is {}", temp).unwrap();
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
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();
    let clocks = rcc.cfgr.use_hse(8.MHz()).freeze(&mut flash.acr);

    let mut gpioa = dp.GPIOA.split();
    let mut gpiob = dp.GPIOB.split();

    let tx_pin = gpioa.pa2.into_alternate_push_pull(&mut gpioa.crl);
    let rx_pin = gpioa.pa3;
    let mut serial = Serial::new(
        dp.USART2,
        (tx_pin, rx_pin),
        &mut afio.mapr,
        Config::default().baudrate(115200.bps()).wordlength_8bits().parity_none().stopbits(StopBits::STOP1),
        &clocks
    );

    let scl_pin = gpiob.pb8.into_alternate_open_drain(&mut gpiob.crh);
    let sda_pin = gpiob.pb9.into_alternate_open_drain(&mut gpiob.crh);
    let _alert_pin = gpioa.pa9;
    let i2c = BlockingI2c::i2c1(
        dp.I2C1,
        (scl_pin, sda_pin),
        &mut afio.mapr,
        Mode::Fast {
            frequency: 400.kHz(),
            duty_cycle: DutyCycle::Ratio16to9,
        },
        clocks,
        1000,
        10,
        1000,
        1000
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

    let mut timer = Timer::syst(cp.SYST, &clocks).counter_hz();
    timer.start(1.Hz()).unwrap();

    let mut mcp9808 = MCP9808::new(i2c);
    let mut temp = mcp9808.read_temperature().unwrap();
    let mut celsius = temp.get_celcius(ResolutionVal::Deg_0_0625C);
    unsafe { uart_send_temp(celsius); }

    loop {
        block!(timer.wait()).unwrap();
        temp = mcp9808.read_temperature().unwrap();
        celsius = temp.get_celcius(ResolutionVal::Deg_0_0625C);
        unsafe { uart_send_temp(celsius); }
    }
}