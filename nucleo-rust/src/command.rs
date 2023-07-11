use heapless::Vec;

pub struct Command {
    pub cmd: CommandCodes,
    pub args: Vec<u8, 4>,
    pub len: usize
}

pub enum RxState {
    Length,
    Data { command: Command, idx: usize },
}

impl Command {
    pub fn new(length: usize) -> Command {
        Command {
            len: length,
            cmd: CommandCodes::NoCommand,
            args: Vec::new()
        }
    }

    pub fn copy(&self, to: &mut Command) {
        to.len = self.len;
        to.cmd = self.cmd;
        to.args = Vec::new();
        for i in 0..self.args.len() {
            to.args.push(self.args[i]).unwrap();
        }
    }
}

#[derive(Copy, Clone)]
pub enum CommandCodes {
    NoCommand = 0,
    DisplayHumidity = 104,
    DisplayKris = 107,
    DisplayLightOn = 108,
    ReadSensors = 114,
    DisplayLightOff = 115,
    DisplayTemperature = 116
}