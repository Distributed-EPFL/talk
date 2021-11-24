use crate::net::Socket;

use tokio::{
    io,
    io::{AsyncWriteExt, WriteHalf},
};

pub(in crate::net) struct UnitSender {
    write_half: WriteHalf<Box<dyn Socket>>,
    buffer: Vec<u8>,
}

impl UnitSender {
    pub fn new(write_half: WriteHalf<Box<dyn Socket>>) -> Self {
        UnitSender {
            write_half,
            buffer: Vec::new(),
        }
    }

    pub fn write_half(&self) -> &WriteHalf<Box<dyn Socket>> {
        &self.write_half
    }

    pub fn as_vec(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        self.send_size(self.buffer.len()).await?;
        self.write_half.write_all(&self.buffer).await?;
        self.buffer.clear();

        Ok(())
    }

    async fn send_size(&mut self, size: usize) -> io::Result<()> {
        let size = (size as u32).to_le_bytes();
        self.write_half.write_all(&size).await
    }
}
