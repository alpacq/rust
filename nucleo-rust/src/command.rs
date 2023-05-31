use heapless::Vec;

pub struct Command {
    pub cmd: u8,
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
            cmd: 0u8,
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