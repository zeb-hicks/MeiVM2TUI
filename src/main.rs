#![allow(clippy::uninlined_format_args)]
extern crate slog;
extern crate slog_scope;
extern crate slog_stdlog;
extern crate slog_term;
#[macro_use]
extern crate log;

use app::*;
use slog::Drain;
use std::fs::OpenOptions;
use std::vec;

// mod opcode;
// mod register;

use meivm2::{vm_write, FlightModule, NavModule, SimulationVM, VMShip, MEM_SHARED_SIZE_U};
use ratatui::crossterm::{event, execute};
use std::sync::mpsc;
use std::time::Duration;

mod app;
mod utils;
mod wavebin;
mod modules;

#[derive(Debug, Clone, Eq, PartialEq)]
enum SimCommand {
  Run,
  Step,
  Halt,
  Reset,
  Restart,
  Debug(bool),
  TickRate(usize),
  Summon,
  Write(u16, u16),
  Read(u16),
  WriteAll(u16, Vec<u16>),
  // ReadAll(Vec<u16>),
  SetUser(u64),
  WriteCommand(String),
  CodeCommand(String),
  Breakpoints(Vec<u16>),
}

#[derive(Debug)]
enum SimOutput {
  MemoryValue(u64, u16, u16),
  MemoryValues(u64, u16, Vec<u16>),
  ChangeUser(u64),
  Error(String),
  SimState(u64, SimStateUpdate),
  ShipState(u64, VMShip),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let log_path = "log.txt";
  let file = OpenOptions::new()
    .append(true)
    .create(true)
    .open(log_path)?;
  let decorator = slog_term::PlainSyncDecorator::new(file);
  let drain = slog_term::FullFormat::new(decorator).build().fuse();
  let logger = slog::Logger::root(drain, slog::o!());

  let _guard = slog_scope::set_global_logger(logger);

  slog_stdlog::init().unwrap();

  info!("File logging started.");

  let terminal = ratatui::init();
  execute!(std::io::stdout(), event::EnableMouseCapture)?;
  // Capture any panics so we can restore the terminal gracefully.
  let _ = std::panic::catch_unwind(|| {
    let mut app = App::new();
    app.run(terminal)
  });
  execute!(std::io::stdout(), event::DisableMouseCapture)?;
  ratatui::restore();
  Ok(())
}

fn sim(sim_rx: mpsc::Receiver<SimCommand>, sim_tx: mpsc::Sender<SimOutput>) {
  let tick_size = Duration::from_millis(50);
  let mut sim_vm = SimulationVM::new();
  let mut active_user: u64 = 0;
  let mut debug_mode: bool = false;
  let mut running: bool = false;
  let mut breakpoints: Vec<u16> = Vec::new();
  let mut tickrate = 64;
  let mut mem: Vec<u16> = vec![0; MEM_SHARED_SIZE_U];
  loop {
    match sim_rx.recv_timeout(tick_size) {
      Ok(v) => {
        let result = || -> Result<(), Box<dyn std::error::Error>> {
          match v {
            SimCommand::Run => {
              running = true;
              sim_vm.user_run(active_user);
            }
            SimCommand::Step => {
              debug_mode = true;
              running = true;
              sim_vm.user_run(active_user);
              let sleep = sim_vm.user_new(active_user).proc.sleep_for;
              sim_vm.tick(sleep.max(1) as usize);
            }
            SimCommand::Halt => {
              running = false;
              sim_vm.user_halt(active_user);
            }
            SimCommand::Reset => {
              debug_mode = false;
              running = false;
              sim_vm.user_reset(active_user);
            }
            SimCommand::Restart => {
              running = false;
              sim_vm.user_restart(active_user);
            }
            SimCommand::Debug(debug) => {
              debug_mode = debug;
            }
            SimCommand::TickRate(rate) => {
              tickrate = rate;
            }
            SimCommand::Read(addr) => {
              let val = sim_vm.user_read(active_user, addr);
              sim_tx.send(SimOutput::MemoryValue(active_user, addr, val))?;
            }
            SimCommand::Write(addr, val) => {
              debug!("Writing value {:04x} to address {:04x} for user {}", val, addr, active_user);
              sim_vm.user_write(active_user, addr, val);
            }
            SimCommand::WriteAll(addr, vals) => {
              debug!("Writing values {:?} to address {:04x} for user {}", vals, addr, active_user);
              for (i, &val) in vals.iter().enumerate() {
                sim_vm.user_write(active_user, addr + i as u16, val);
              }
            }
            SimCommand::WriteCommand(vals) => {
              let vals = &mut vals.split_whitespace();
              vals.next();
              let vmproc = &mut sim_vm.make_user(active_user).proc;
              vm_write(vals, vmproc.as_mut(), 0);
              // write_from_input(&mut sim_vm, 0, &vals);
            }
            SimCommand::CodeCommand(vals) => {
              let vals = &mut vals.split_whitespace();
              vals.next();
              let vmproc = &mut sim_vm.make_user(active_user).proc;
              vm_write(vals, vmproc.as_mut(), 0x40);
              // write_from_input(&mut sim_vm, 0x40, &vals);
            }
            SimCommand::Breakpoints(bps) => {
              breakpoints = bps;
              let vmproc = &mut sim_vm.make_user(active_user).proc;
              vmproc.breakpoints = breakpoints.clone().iter().map(|&x| (0u64, x)).collect();
            }
            // SimCommand::ReadAll(vals) => {
            //   let mem = vals.iter().map(|addr| {
            //     sim_vm.user_peek(active_user, *addr)
            //   }).collect::<Vec<u16>>();
            //   sim_tx.send(SimOutput::MemoryValues(active_user, 0, mem))?;
            // }
            SimCommand::SetUser(user) => {
              active_user = user;
              sim_tx.send(SimOutput::ChangeUser(user))?;
            }
            SimCommand::Summon => {
              sim_vm.user_new(active_user);
            }
          }
          for i in 0..MEM_SHARED_SIZE_U {
            mem[i] = sim_vm.user_read(active_user, i as u16);
          }
          sim_tx.send(SimOutput::MemoryValues(active_user, 0, mem.clone()))?;

          // let halt_reason =

          let user = &mut sim_vm.make_user(active_user);
          sim_tx.send(SimOutput::SimState(active_user, SimStateUpdate {
            running: user.proc.is_running,
            debug_mode: debug_mode,
            sleep: user.proc.sleep_for,
            defer: user.proc.defer.is_some(),
            halt_reason: None, // TODO
          })).unwrap();
          Ok(())
        }();
        if let Err(message) = result {
          sim_tx.send(SimOutput::Error(format!("Error: {}", message))).unwrap();
          continue;
        }
      }
      Err(mpsc::RecvTimeoutError::Timeout) => (),
      Err(mpsc::RecvTimeoutError::Disconnected) => return
    }
    if running && !debug_mode {
      sim_vm.tick(tickrate);
      if let Some(&proc) = sim_vm.processes.front() {
        // Get the current breakpoint if any
        let proc = unsafe { &*proc };
        if proc.proc.current_breakpoint.is_some() {
          debug_mode = true;

        }
      }

      // let pc = sim_vm.user_peek(active_user, 0x3c);
      // if breakpoints.iter().position(|&x| x == pc).is_some() {
      //   sim_vm.user_halt(active_user);
      //   sim_tx.send(SimOutput::DebugPrint(active_user, format!("Breakpoint hit at {:04x}", pc))).unwrap();
      // }

      // TODO: This could be smarter by sending only the dirty memory regions
      // let mem = sim_vm.user_read(active_user, 0, MEM_SHARED_SIZE_U);
      // for i in 0..MEM_SHARED_SIZE_U {
      //   mem[i] = user.proc.read_mem(&mut ctx, i as u16);
      // }
      for i in 0..MEM_SHARED_SIZE_U {
        mem[i] = sim_vm.user_read(active_user, i as u16);
      }

      sim_tx.send(SimOutput::MemoryValues(active_user, 0, mem.clone())).unwrap();
      if let Some(user) = sim_vm.find_user(active_user) {
        sim_tx.send(SimOutput::SimState(active_user, SimStateUpdate {
          running: user.proc.is_running,
          debug_mode: debug_mode,
          sleep: user.proc.sleep_for,
          defer: user.proc.defer.is_some(),
          halt_reason: None, // TODO: Halt support
        })).unwrap();
        // user.context.halt_reason = None;
      }
    }
    let user = sim_vm.make_user(active_user);
    let ship = VMShip {
      flight: FlightModule {
        ..user.ship.flight
      },
      nav: NavModule {
        ..user.ship.nav
      },
      ..user.ship
    };
    sim_tx.send(SimOutput::ShipState(active_user, ship)).unwrap();
  }
}
