use core::panic;
use std::{io::Read, vec};

pub struct WaveVMBin {
  pub mem: Vec<u16>,
  pub code: Vec<u16>,
}

pub fn load_wavevm_bin(file: &str) -> Result<WaveVMBin, std::io::Error> {
  let mut file = std::fs::File::open(file)?;
  let mut buffer = Vec::new();
  file.read_to_end(&mut buffer)?;

  // Check for magic numbers
  if &buffer[0..4] != b"MWvm" {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Invalid magic number",
    ));
  }

  let version = buffer[4] as usize;

  match version {
    1 => {
      let mem_start = buffer[5] as usize;
      let code_start = buffer[6] as usize;

      let mem_size = code_start - mem_start;
      let code_size = buffer.len() - code_start;

      let mut mem = vec![0; mem_size];
      let mut code = vec![0; code_size];

      if mem_size % 2 != 0 {
        panic!("Memory size is not even. {}, {}, {}, {}", mem_start, mem_size, code_start, code_size);
      }
      for i in (0..mem_size).step_by(2) {
        mem[i / 2] = u16::from_be_bytes([buffer[mem_start + i], buffer[mem_start + i + 1]]);
      }

      if code_size % 2 != 0 {
        panic!("Code size is not even. {}, {}, {}, {}", mem_start, mem_size, code_start, code_size);
      }
      for i in (0..code_size).step_by(2) {
        code[i / 2] = u16::from_be_bytes([buffer[code_start + i], buffer[code_start + i + 1]]);
      }

      Ok(WaveVMBin { mem, code })
    }
    _ => {
      error!("Unsupported version: {}", version);
      Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "Unsupported version",
      ))
    }
  }
}
