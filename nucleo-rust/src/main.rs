//#![deny(unsafe_code)]
#![no_std]
#![no_main]

mod command;

use panic_halt as _;
use cortex_m_rt::entry;
use stm32f1xx_hal::{
    adc,
    pac::{self, interrupt, USART2, SPI2},
    prelude::*,
    gpio::{Pin, Output, Alternate},
    spi::{self, Spi, Spi2NoRemap},
    serial::{Config, Serial, StopBits, Tx, Rx}};
use core::fmt::Write;
use heapless::{Vec, String};
use command::{Command, RxState, CommandCodes};
use lcd_hal::{Display, pcd8544::spi::Pcd8544Spi};
use dht11::{Dht11, Measurement};

static mut RX: Option<Rx<USART2>> = None;
static mut TX: Option<Tx<USART2>> = None;
static mut CURRENT_COMMAND: Command = Command { len: 1, cmd: CommandCodes::NoCommand, args: Vec::new() };
static mut RX_STATE: RxState = RxState::Length;
static mut DISPLAY: Option<Pcd8544Spi<Spi<SPI2, Spi2NoRemap, (Pin<'B', 13, Alternate>, Pin<'B', 14>, Pin<'B', 15, Alternate>), u8>, Pin<'C', 7, Output>, Pin<'B', 10, Output>>> = None;
static mut LIGHT: Option<Pin<'A', 10, Output>> = None;
static mut DHT11_MEASUREMENT: Option<Measurement> = None;
static mut MQ7_MEASUREMENT: u16 = 0u16;
static DEBUG_MODE: bool = false;

unsafe fn uart_command_response() {
    if let Some(tx) = TX.as_mut() {
        writeln!(tx, "Length of cmd is {}\r", CURRENT_COMMAND.len).unwrap();
        writeln!(tx, "Command code is {}\r", (CURRENT_COMMAND.cmd as u8)).unwrap();
        for i in 0..CURRENT_COMMAND.args.len() {
            writeln!(tx, "Argument {} is {}\r", i, CURRENT_COMMAND.args[i]).unwrap();
        }
    }
}

unsafe fn execute_command() {
    match CURRENT_COMMAND.cmd {
        CommandCodes::DisplayGas => { //g => read gas
            if let Some(display) = DISPLAY.as_mut() {
                display.clear().unwrap();
                let _res = display.print(b"Gas reading: ").unwrap();
                let _res = display.set_position(0u8, 1u8).unwrap();
                let gas: String<5> = String::from(MQ7_MEASUREMENT);
                let _res = display.print(gas.as_bytes()).unwrap();
            }
        }
        CommandCodes::DisplayHumidity => { //h => read humidity
            if let Some(measurement) = DHT11_MEASUREMENT.as_mut() {
                let hum_full: String<2> = String::from(measurement.humidity / 10);
                let hum_frac: String<1> = String::from(measurement.humidity % 10);
                if let Some(display) = DISPLAY.as_mut() {
                    display.clear().unwrap();
                    let _res = display.print(b"Humidity: ").unwrap();
                    let _res = display.set_position(0u8, 1u8).unwrap();
                    let _res = display.print(hum_full.as_bytes()).unwrap();
                    let _res = display.print(b".").unwrap();
                    let _res = display.print(hum_frac.as_bytes()).unwrap();
                    let _res = display.print(b"%").unwrap();
                }
            }
        }
        CommandCodes::DisplayKris => { //k => changes displayed string
            if let Some(display) = DISPLAY.as_mut() {
                display.clear().unwrap();
                let _res = display.print(b"Hello Kris").unwrap();
            }
        }
        CommandCodes::DisplayLightOn => { //l => turn on display's BL
            if let Some(light) = LIGHT.as_mut() {
                light.set_high();
            }
        }
        CommandCodes::ReadSensors => { //r => read measurements, [g,h,t]
            if let Some(tx) = TX.as_mut() {
                for i in 0..CURRENT_COMMAND.args.len() {
                    match CURRENT_COMMAND.args[i] {
                        103 => { //g
                            writeln!(tx, "Gas sensor reading is {}\r", MQ7_MEASUREMENT).unwrap();
                        }
                        104 => { //h
                            if let Some(measurement) = DHT11_MEASUREMENT.as_mut() {
                                let hum_full: String<2> = String::from(measurement.humidity / 10);
                                let hum_frac: String<1> = String::from(measurement.humidity % 10);
                                writeln!(tx, "Humidity is {}.{}%\r", hum_full, hum_frac).unwrap();
                            }
                        }
                        116 => { //t
                            if let Some(measurement) = DHT11_MEASUREMENT.as_mut() {
                                let temp_full: String<2> = String::from(measurement.temperature / 10);
                                let temp_frac: String<1> = String::from(measurement.temperature % 10);
                                writeln!(tx, "Temperature is {}.{}oC\r", temp_full, temp_frac).unwrap();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        CommandCodes::DisplayLightOff => { //s => turn off display's BL
            if let Some(light) = LIGHT.as_mut() {
                light.set_low();
            }
        }
        CommandCodes::DisplayTemperature => { //t => read temperature
            if let Some(measurement) = DHT11_MEASUREMENT.as_mut() {
                let temp_full: String<2> = String::from(measurement.temperature / 10);
                let temp_frac: String<1> = String::from(measurement.temperature % 10);
                if let Some(display) = DISPLAY.as_mut() {
                    display.clear().unwrap();
                    let _res = display.print(b"Temperature: ").unwrap();
                    let _res = display.set_position(0u8, 1u8).unwrap();
                    let _res = display.print(temp_full.as_bytes()).unwrap();
                    let _res = display.print(b".").unwrap();
                    let _res = display.print(temp_frac.as_bytes()).unwrap();
                    let _res = display.print(b"oC").unwrap();
                }
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
                                match received {
                                    103 => command.cmd = CommandCodes::DisplayGas,
                                    104 => command.cmd = CommandCodes::DisplayHumidity,
                                    107 => command.cmd = CommandCodes::DisplayKris,
                                    108 => command.cmd = CommandCodes::DisplayLightOn,
                                    114 => command.cmd = CommandCodes::ReadSensors,
                                    115 => command.cmd = CommandCodes::DisplayLightOff,
                                    116 => command.cmd = CommandCodes::DisplayTemperature,
                                    _ => command.cmd = CommandCodes::NoCommand
                                }
                            } else {
                                command.args.push(received as u8).unwrap();
                            }
                            *idx += 1;
                            if *idx == command.len {
                                command.copy(&mut CURRENT_COMMAND);
                                RX_STATE = RxState::Length;
                                execute_command();
                                if DEBUG_MODE {
                                    uart_command_response();
                                }
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
    //basic structures
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain();

    //clock configuration
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .freeze(&mut flash.acr);

    //GPIO banks
    let mut gpioa = dp.GPIOA.split();
    let mut gpiob = dp.GPIOB.split();
    let mut gpioc = dp.GPIOC.split();

    let mut delay = cp.SYST.delay(&clocks);

    //timer configuration
    let mut timer = dp.TIM2.counter_hz(&clocks);

    //SPI configuration & LCD display pins
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

    bl.set_high();

    //UART configuration
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

    //LCD display creation & test
    let mut display = Pcd8544Spi::new(spi, dc, cs, &mut rst, &mut delay).unwrap();

    let res = display.print(b"Hello world");
    match res {
        Ok(_) => writeln!(serial.tx, "Write performed\r\n").unwrap(),
        Err(_) => writeln!(serial.tx, "Write failed\r\n").unwrap()
    };

    //DHT11 humidity & temperature sensor configuration
    let dht11_pin = gpiob.pb2.into_open_drain_output(&mut gpiob.crl);

    let mut dht11 = Dht11::new(dht11_pin);

    let mut measurement = Measurement {temperature: 0i16, humidity: 0u16};

    match dht11.perform_measurement(&mut delay) {
        Ok(msrmt) => measurement = msrmt,
        Err(e) => writeln!(serial.tx, "Error: {:?}\r\n", e).unwrap(),
    };

    //MQ7 configuration - ADC and digital alarm input
    let mut adc = adc::Adc::adc1(dp.ADC1, clocks);
    let mut ch0 = gpioa.pa0.into_analog(&mut gpioa.crl);

    //let mut mq7_pin = gpioc.pc15.into_floating_input(&mut gpioc.crh);

    writeln!(serial.tx, "Please type command |len||cmd||args..|:\r\n").unwrap();

    //start timer
    timer.start(1.Hz()).unwrap();

    writeln!(serial.tx, "Timer started\r\n").unwrap();

    //static global variables write
    cortex_m::interrupt::free(|_| unsafe {
        TX.replace(serial.tx);
        RX.replace(serial.rx);
        DISPLAY.replace(display);
        LIGHT.replace(bl);
        DHT11_MEASUREMENT.replace(measurement);
    });

    //enable interrupts
    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USART2);
    }

    unsafe {
        if let Some(serial_tx) = TX.as_mut() {
            writeln!(serial_tx, "Interrupts enabled\r\n").unwrap();
        }
    }

    loop {
        timer.wait().unwrap();
        unsafe {
            MQ7_MEASUREMENT = adc.read(&mut ch0).unwrap();
            match dht11.perform_measurement(&mut delay) {
                Ok(msrmt) => {
                    if let Some(meas) = DHT11_MEASUREMENT.as_mut() {
                        *meas = msrmt;
                    }
                }
                Err(e) => {
                    if let Some(serial_tx) = TX.as_mut() {
                        writeln!(serial_tx, "Error: {:?}\r\n", e).unwrap();
                    }
                }
            };
        }
    }
}