use ratatui::{style::{Color, Stylize as _}, text::Span};

use crate::S;


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Module {
  Control(u16),
  Flight(u16),
  Nav(u16),
  Radar(u16),
  ConstStore(u16),
}

impl Module {
  pub fn addr_to_slot(addr: u16) -> Option<usize> {
    if (addr < 0x300) || (addr > 0x3ff) {
      return None;
    }
    let addr = addr.saturating_sub(0x300);
    let slot = addr / 0x20;
    if slot > 7 {
      return None;
    }
    Some(slot as usize)
  }
  pub fn type_from_id(id: u16, slot: u16) -> Option<Self> {
    match id {
      0x0 => Some(Module::Control(slot)),
      0x1000..=0x1007 => Some(Module::ConstStore(slot)),
      0x4000 => Some(Module::Flight(slot)),
      0x4040..=0x4047 => Some(Module::Radar(slot)),
      0x4050..=0x4057 => Some(Module::Nav(slot)),
      _ => None,
    }
  }
  pub fn module_register_info<'a>(self, addr: usize) -> Option<(Span<'a>, Span<'a>)> {
    if (addr < 0x300) || (addr > 0x3ff) {
      return None;
    }
    let addr = match self {
      Module::Control(slot) |
      Module::Flight(slot) |
      Module::Nav(slot) |
      Module::Radar(slot) => {
        addr - (0x300 + slot as usize * 0x20)
      }
      _ => addr
    };
    match self {
      Module::Control(_) => {
        match addr {
          0x00 => Some((S!("CSTA").fg(Color::Rgb(128, 128, 128)), S!("Core Status").gray())),
          0x01 => Some((S!("CID").fg(Color::Rgb(128, 128, 128)), S!("Core ID").gray())),
          0x02 => Some((S!("CPRT").fg(Color::Rgb(128, 128, 128)), S!("Core Exception Register").gray())),

          0x04 => Some((S!("CCRL").fg(Color::Rgb(128, 128, 128)), S!("Core Instruction Register").gray())),

          0x05 => Some((S!("CUID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x06 => Some((S!("CUID").fg(Color::Rgb(128, 128, 128)), S!("Core User ID").gray())),
          0x07 => Some((S!("CUID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x0c => Some((S!("TID").fg(Color::Rgb(128, 128, 128)), S!("Current Thread ID").gray())),
          0x0d => Some((S!("TPRT").fg(Color::Rgb(128, 128, 128)), S!("Thread Protection").gray())),
          0x10 => Some((S!("TBK0").fg(Color::Rgb(128, 128, 128)), S!("Bank Select 0").gray())),
          0x14 => Some((S!("TBK1").fg(Color::Rgb(128, 128, 128)), S!("Bank Select 1").gray())),

          0x18 => Some((S!("TMS1").fg(Color::Rgb(128, 128, 128)), S!("Module Select 1").gray())),
          0x19 => Some((S!("CMS1").fg(Color::Rgb(128, 128, 128)), S!("Module Select 1").gray())),
          0x1a => Some((S!("CMS2").fg(Color::Rgb(128, 128, 128)), S!("Module Select 2").gray())),
          0x1b => Some((S!("CMS3").fg(Color::Rgb(128, 128, 128)), S!("Module Select 3").gray())),
          0x1c => Some((S!("CMS4").fg(Color::Rgb(128, 128, 128)), S!("Module Select 4").gray())),
          0x1d => Some((S!("CMS5").fg(Color::Rgb(128, 128, 128)), S!("Module Select 5").gray())),
          0x1e => Some((S!("CMS6").fg(Color::Rgb(128, 128, 128)), S!("Module Select 6").gray())),
          0x1f => Some((S!("CMS7").fg(Color::Rgb(128, 128, 128)), S!("Module Select 7").gray())),
          _ => Some((S!("").fg(Color::Rgb(64, 64, 64)), S!("0").fg(Color::Rgb(64, 64, 64)))),
        }
      }
      Module::Flight(_) => {
        match addr {
          0x00 => Some((S!("MSTS").fg(Color::Rgb(128, 128, 128)), S!("Module Status").gray())),
          0x01 => Some((S!("MMID").fg(Color::Rgb(128, 128, 128)), S!("Module ID").gray())),

          0x04 => Some((S!("RRVx").fg(Color::Rgb(128, 128, 128)), S!("Req. Vx").gray())),
          0x05 => Some((S!("RRVy").fg(Color::Rgb(128, 128, 128)), S!("Req. Vy").gray())),
          0x06 => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x07 => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),

          0x08 => Some((S!("CRVx").fg(Color::Rgb(128, 128, 128)), S!("Current Req. Vx").gray())),
          0x09 => Some((S!("CRVy").fg(Color::Rgb(128, 128, 128)), S!("Current Req. Vy").gray())),

          0x0c => Some((S!("RH").fg(Color::Rgb(128, 128, 128)), S!("Req. Heading").gray())),
          0x0d => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x0e => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x0f => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),

          0x10 => Some((S!("CAH").fg(Color::Rgb(128, 128, 128)), S!("Abs. Heading").gray())),

          0x14 => Some((S!("EEN").fg(Color::Rgb(128, 128, 128)), S!("Engine Flags").gray())),
          0x15 => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x16 => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x17 => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),

          0x1c => Some((S!("SHCC").fg(Color::Rgb(128, 128, 128)), S!("Ship Color").gray())),
          0x1d => Some((S!("SHCM").fg(Color::Rgb(128, 128, 128)), S!("Ship Alpha Mode").gray())),
          _ => Some((S!("").fg(Color::Rgb(64, 64, 64)), S!("0").fg(Color::Rgb(64, 64, 64)))),
        }
      }
      Module::Nav(_) => {
        match addr {
          0x00 => Some((S!("MSTS").fg(Color::Rgb(128, 128, 128)), S!("Module Status").gray())),
          0x01 => Some((S!("MMID").fg(Color::Rgb(128, 128, 128)), S!("Module ID").gray())),

          0x04 => Some((S!("NASx").fg(Color::Rgb(128, 128, 128)), S!("Abs. Screen X").gray())),
          0x05 => Some((S!("NASy").fg(Color::Rgb(128, 128, 128)), S!("Abs. Screen Y").gray())),

          0x08 => Some((S!("NTSx").fg(Color::Rgb(128, 128, 128)), S!("Target Abs. X").gray())),
          0x09 => Some((S!("NTSy").fg(Color::Rgb(128, 128, 128)), S!("Target Abs. Y").gray())),
          0x0a => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),
          0x0b => Some((S!("M").fg(Color::Rgb(128, 128, 128)), S!("Scratch").blue())),

          0x0c => Some((S!("NTGT").fg(Color::Rgb(128, 128, 128)), S!("Target Selector").gray())),

          0x0d => Some((S!("NTGI").fg(Color::Rgb(64, 64, 64)),    S!("╭───────╮").fg(Color::Rgb(64, 64, 64)))),
          0x0e => Some((S!("NTGI").fg(Color::Rgb(128, 128, 128)), S!("Target ID").gray())),
          0x0f => Some((S!("NTGI").fg(Color::Rgb(64, 64, 64)),    S!("╰───────╯").fg(Color::Rgb(64, 64, 64)))),

          0x10 => Some((S!("NRDx").fg(Color::Rgb(128, 128, 128)), S!("Target Rel. X").gray())),
          0x11 => Some((S!("NRDy").fg(Color::Rgb(128, 128, 128)), S!("Target Rel. Y").gray())),

          0x14 => Some((S!("NRVx").fg(Color::Rgb(128, 128, 128)), S!("Target Rel. Vx").gray())),
          0x15 => Some((S!("NRVy").fg(Color::Rgb(128, 128, 128)), S!("Target Rel. Vy").gray())),

          0x18 => Some((S!("NAHT").fg(Color::Rgb(128, 128, 128)), S!("T.Abs. Heading Toward").gray())),
          0x1a => Some((S!("NAHF").fg(Color::Rgb(128, 128, 128)), S!("T.Abs. Heading Away").gray())),

          0x1c => Some((S!("NRHT").fg(Color::Rgb(128, 128, 128)), S!("T.Rel. Heading Toward").gray())),
          0x1e => Some((S!("NRHF").fg(Color::Rgb(128, 128, 128)), S!("T.Rel. Heading Away").gray())),
          _ => Some((S!("").fg(Color::Rgb(64, 64, 64)), S!("0").fg(Color::Rgb(64, 64, 64)))),
        }
      }
      Module::Radar(_) => {
        match addr {
          0x00 => Some((S!("MSTS").fg(Color::Rgb(128, 128, 128)), S!("Module Status").gray())),
          0x01 => Some((S!("MMID").fg(Color::Rgb(128, 128, 128)), S!("Module ID").gray())),

          0x04 => Some((S!("RSSH").fg(Color::Rgb(128, 128, 128)), S!("Select Scan Heading").gray())),
          0x06 => Some((S!("RHLS").fg(Color::Rgb(128, 128, 128)), S!("Last Scan Heading").gray())),
          0x07 => Some((S!("RNSR").fg(Color::Rgb(128, 128, 128)), S!("Signature Count").gray())),

          0x08 => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(220, 0, 0)))),
          0x09 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x0a => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(220, 0, 0)))),
          0x0b => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x0c => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(220, 160, 0)))),
          0x0d => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x0e => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(220, 160, 0)))),
          0x0f => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x10 => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(220, 220, 0)))),
          0x11 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x12 => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(220, 220, 0)))),
          0x13 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x14 => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(40, 220, 40)))),
          0x15 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x16 => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(40, 220, 40)))),
          0x17 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x18 => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(60, 80, 220)))),
          0x19 => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x1a => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(60, 80, 220)))),
          0x1b => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),

          0x1c => Some((S!("RSDT").fg(Color::Rgb(128, 128, 128)), S!("Signature Distance").fg(Color::Rgb(140, 80, 200)))),
          0x1d => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╭──────────╮").fg(Color::Rgb(64, 64, 64)))),
          0x1e => Some((S!("RSID").fg(Color::Rgb(128, 128, 128)), S!("Signature ID").fg(Color::Rgb(140, 80, 200)))),
          0x1f => Some((S!("RSID").fg(Color::Rgb(64, 64, 64)),    S!("╰──────────╯").fg(Color::Rgb(64, 64, 64)))),
          _ => Some((S!("").fg(Color::Rgb(64, 64, 64)), S!("0").fg(Color::Rgb(64, 64, 64)))),
        }
      }
      Module::ConstStore(_) => {
        match addr {
          0x00 => Some((S!("CS0x").fg(Color::Rgb(128, 128, 128)), S!("Constant c0.x").gray())),
          0x01 => Some((S!("CS0y").fg(Color::Rgb(128, 128, 128)), S!("Constant c0.y").gray())),
          0x02 => Some((S!("CS0z").fg(Color::Rgb(128, 128, 128)), S!("Constant c0.z").gray())),
          0x03 => Some((S!("CS0w").fg(Color::Rgb(128, 128, 128)), S!("Constant c0.w").gray())),

          0x04 => Some((S!("CS1x").fg(Color::Rgb(128, 128, 128)), S!("Constant c1.x").gray())),
          0x05 => Some((S!("CS1y").fg(Color::Rgb(128, 128, 128)), S!("Constant c1.y").gray())),
          0x06 => Some((S!("CS1z").fg(Color::Rgb(128, 128, 128)), S!("Constant c1.z").gray())),
          0x07 => Some((S!("CS1w").fg(Color::Rgb(128, 128, 128)), S!("Constant c1.w").gray())),

          0x08 => Some((S!("CS2x").fg(Color::Rgb(128, 128, 128)), S!("Constant c2.x").gray())),
          0x09 => Some((S!("CS2y").fg(Color::Rgb(128, 128, 128)), S!("Constant c2.y").gray())),
          0x0a => Some((S!("CS2z").fg(Color::Rgb(128, 128, 128)), S!("Constant c2.z").gray())),
          0x0b => Some((S!("CS2w").fg(Color::Rgb(128, 128, 128)), S!("Constant c2.w").gray())),

          0x0c => Some((S!("CS3x").fg(Color::Rgb(128, 128, 128)), S!("Constant c3.x").gray())),
          0x0d => Some((S!("CS3y").fg(Color::Rgb(128, 128, 128)), S!("Constant c3.y").gray())),
          0x0e => Some((S!("CS3z").fg(Color::Rgb(128, 128, 128)), S!("Constant c3.z").gray())),
          0x0f => Some((S!("CS3w").fg(Color::Rgb(128, 128, 128)), S!("Constant c3.w").gray())),

          0x10 => Some((S!("CS4x").fg(Color::Rgb(128, 128, 128)), S!("Constant c4.x").gray())),
          0x11 => Some((S!("CS4y").fg(Color::Rgb(128, 128, 128)), S!("Constant c4.y").gray())),
          0x12 => Some((S!("CS4z").fg(Color::Rgb(128, 128, 128)), S!("Constant c4.z").gray())),
          0x13 => Some((S!("CS4w").fg(Color::Rgb(128, 128, 128)), S!("Constant c4.w").gray())),

          0x14 => Some((S!("CS5x").fg(Color::Rgb(128, 128, 128)), S!("Constant c5.x").gray())),
          0x15 => Some((S!("CS5y").fg(Color::Rgb(128, 128, 128)), S!("Constant c5.y").gray())),
          0x16 => Some((S!("CS5z").fg(Color::Rgb(128, 128, 128)), S!("Constant c5.z").gray())),
          0x17 => Some((S!("CS5w").fg(Color::Rgb(128, 128, 128)), S!("Constant c5.w").gray())),

          0x18 => Some((S!("CS6x").fg(Color::Rgb(128, 128, 128)), S!("Constant c6.x").gray())),
          0x19 => Some((S!("CS6y").fg(Color::Rgb(128, 128, 128)), S!("Constant c6.y").gray())),
          0x1a => Some((S!("CS6z").fg(Color::Rgb(128, 128, 128)), S!("Constant c6.z").gray())),
          0x1b => Some((S!("CS6w").fg(Color::Rgb(128, 128, 128)), S!("Constant c6.w").gray())),

          0x1c => Some((S!("CS7x").fg(Color::Rgb(128, 128, 128)), S!("Constant c7.x").gray())),
          0x1d => Some((S!("CS7y").fg(Color::Rgb(128, 128, 128)), S!("Constant c7.y").gray())),
          0x1e => Some((S!("CS7z").fg(Color::Rgb(128, 128, 128)), S!("Constant c7.z").gray())),
          0x1f => Some((S!("CS7w").fg(Color::Rgb(128, 128, 128)), S!("Constant c7.w").gray())),
          _ => Some((S!("").fg(Color::Rgb(64, 64, 64)), S!("0").fg(Color::Rgb(64, 64, 64)))),
        }
      }
    }
  }
}
