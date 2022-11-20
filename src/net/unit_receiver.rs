use crate::net::Socket;
use std::mem;
use tokio::{
    io,
    io::{AsyncReadExt, ReadHalf},
};

pub(in crate::net) struct UnitReceiver {
    read_half: ReadHalf<Box<dyn Socket>>,
    buffer: Vec<u8>,
}

impl UnitReceiver {
    pub fn new(read_half: ReadHalf<Box<dyn Socket>>) -> Self {
        UnitReceiver {
            read_half,
            buffer: Vec::new(),
        }
    }

    pub fn read_half(&self) -> &ReadHalf<Box<dyn Socket>> {
        &self.read_half
    }

    pub fn as_slice(&self) -> &[u8] {
        self.buffer.as_slice()
    }

    pub fn as_vec(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    pub fn free_buffer(&mut self) {
        mem::take(&mut self.buffer);
    }

    pub async fn receive(&mut self) -> io::Result<()> {
        let size = self.receive_size().await?;
        self.buffer.resize(size, 0);
        self.read_half.read_exact(&mut self.buffer[..]).await?;

        Ok(())
    }

    async fn receive_size(&mut self) -> io::Result<usize> {
        let mut size = [0; mem::size_of::<u32>()];
        self.read_half.read_exact(&mut size[..]).await?;
        Ok(u32::from_le_bytes(size) as usize)
    }
}
