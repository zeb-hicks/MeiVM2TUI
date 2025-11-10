use meivm2::opcode::{Opcode, RegIndex};
use meivm2::{FlightModule, MEM_SHARED_SIZE_U, Ship};
use ratatui::crossterm::event;
use ratatui::layout::{Alignment, Margin, Position, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Clear, Padding, Paragraph};
use ratatui::{DefaultTerminal, Frame};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crate::modules::Module;
use crate::{sim, SimCommand, SimOutput, S};
use crate::utils::*;
use crate::wavebin::*;

use clap::Parser as _;

#[derive(clap::Parser)]
#[command(about = "WaveVM Assembly Compiler", long_about = None)]
struct Cli {
  /// Input file to load into memory
  #[arg()]
  infile: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum InputMode {
  #[default]
  Menu,
  Command,
}

#[derive(Debug, Clone, Copy)]
pub enum ViewMode {
  Log,
  Memory,
  Code,
}

impl ViewMode {
  fn next(self) -> Self {
    match self {
      ViewMode::Log => ViewMode::Memory,
      ViewMode::Memory => ViewMode::Code,
      ViewMode::Code => ViewMode::Log,
    }
  }

  fn prev(self) -> Self {
    match self {
      ViewMode::Log => ViewMode::Code,
      ViewMode::Memory => ViewMode::Log,
      ViewMode::Code => ViewMode::Memory,
    }
  }
}

pub struct SimState {
  memory: [u16; MEM_SHARED_SIZE_U],
  running: bool,
  sleep: u32,
  defer: bool,
  debug_mode: bool,
  active_user: u64,
}

#[derive(Debug)]
pub struct SimStateUpdate {
  pub running: bool,
  pub debug_mode: bool,
  pub sleep: u32,
  pub defer: bool,
  pub halt_reason: Option<String>,
}

pub struct App {
  sim_state: SimState,
  ship: Ship,
  ui_regions: UIRegions,
  mouse_pos: Option<Position>,
  mouse_clicks: Vec<Position>,
  // mouse_drag: Option<(Position, Position)>,
  mouse_drops: Vec<(Position, Position)>,
  view_mode: ViewMode,
  input_mode: InputMode,
  input_string: String,
  input_cursor: usize,
  log_strings: Vec<Vec<ColoredString>>,
  log_position: usize,
  code_scroll: usize,
  code_offset: usize,
  memory_scroll: usize,
  breakpoints: Vec<u16>,
  watch_addr: Vec<(u16, u16, Option<String>)>,
  actions: Vec<AppActions>,
}

pub enum AppActions {
  Breakpoint(u16),
}

impl App {
  pub fn new() -> Self {
    App {
      sim_state: SimState {
        memory: [0; MEM_SHARED_SIZE_U],
        running: false,
        sleep: 0,
        defer: false,
        debug_mode: false,
        active_user: 0,
      },
      ship: Ship {
        flight: FlightModule::default(),
        ..Default::default()
      },
      ui_regions: UIRegions::default(),
      mouse_pos: None,
      mouse_clicks: Vec::new(),
      // mouse_drag: None,
      mouse_drops: Vec::new(),
      view_mode: ViewMode::Code,
      input_mode: InputMode::Menu,
      input_string: String::new(),
      input_cursor: 0,
      log_strings: Vec::new(),
      log_position: 0,
      code_scroll: 0,
      code_offset: 0x40,
      memory_scroll: 0,
      breakpoints: Vec::new(),
      watch_addr: vec![
        (0x380, 0x8, Some(S!("Ship"))),
        (0x3c0, 0x8, Some(S!("NAV"))),
        (0x80, 0x20, None),
        (0x1000, 0x30, Some(S!("Public Memory"))),
      ],
      actions: Vec::new(),
    }
  }

  pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    let (sim_channel_tx, sim_channel_rx) = mpsc::sync_channel(16);
    let (sim_output_tx, sim_output_rx) = mpsc::channel();
    std::thread::spawn(|| {
      sim(sim_channel_rx, sim_output_tx)
    });

    // let (ship_state_tx, ship_state_rx) = mpsc::channel();
    // let (ship_image_tx, ship_image_rx) = mpsc::channel();
    // std::thread::spawn(|| {
    //   ship_image_generator(ship_state_rx, ship_image_tx);
    // });

    sim_channel_tx.send(SimCommand::Reset)?;
    sim_channel_tx.send(SimCommand::Debug(true))?;
    sim_channel_tx.send(SimCommand::Run)?;

    if let Some(infile) = args.infile {
      let infile = infile.to_str().unwrap();
      let bin = load_wavevm_bin(infile);
      match bin {
        Ok(bin) => {
          // sim_channel_tx.send(SimCommand::Summon)?;
          // sim_channel_tx.send(SimCommand::SetUser(0))?;
          sim_channel_tx.send(SimCommand::WriteAll(0, bin.mem))?;
          sim_channel_tx.send(SimCommand::WriteAll(0x40, bin.code))?;
        }
        Err(err) => {
          self.print_plain(format!("Failed to load file: {}", err));
        }
      }
    }

    sim_channel_tx.send(SimCommand::Halt)?;
    sim_channel_tx.send(SimCommand::Debug(false))?;

    let mut mouse_down: Position = Position { x: 0, y: 0 };

    loop {
      terminal.draw(|frame| self.draw(frame))?;

      // if let Ok(output) = ship_image_rx.try_recv() {
      //   self.ship_image = Some(output);
      // }

      use InputMode::*;
      use event::KeyCode as K;
      use event::KeyModifiers as M;
      let mut exit = false;

      while let Some(action) = self.actions.pop() {
        match action {
          AppActions::Breakpoint(addr) => {
            if let Some(i) = self.breakpoints.iter().position(|&x| x == addr) {
              self.breakpoints.remove(i);
            } else {
              self.breakpoints.push(addr);
            }
            sim_channel_tx.send(SimCommand::Breakpoints(self.breakpoints.clone()))?;
          }
        }
      }

      while matches!(event::poll(Duration::ZERO), Ok(true)) {
        match event::read()? {
          event::Event::Paste(text) => {
            for c in text.chars() {
              self.input_new_char(c);
            }
          }
          event::Event::Key(event) => {
            if event.code == K::Char('c') && event.modifiers == M::CONTROL {
              exit = true;
            }
            match (self.input_mode, event.code) {
              (Menu, K::Char(' ')) => { self.input_mode = InputMode::Command; }
              (Menu, K::Char('r')) => { self.sim_state.running = true; sim_channel_tx.send(SimCommand::Run)?; }
              (Menu, K::Char('s')) => { self.sim_state.running = false; self.sim_state.debug_mode = true; sim_channel_tx.send(SimCommand::Step)?; }
              (Menu, K::Char('R')) => { self.sim_state.running = false; sim_channel_tx.send(SimCommand::Halt)?; }
              (Menu, K::Char('d')) => { self.sim_state.debug_mode = !self.sim_state.debug_mode; sim_channel_tx.send(SimCommand::Debug(self.sim_state.debug_mode))?; }
              (Menu, K::Char('e')) => { self.sim_state.running = false; sim_channel_tx.send(SimCommand::Restart)?; }
              (Menu, K::Tab) => { self.view_mode = self.view_mode.next(); }
              (Menu, K::BackTab) => { self.view_mode = self.view_mode.prev(); }
              (Command, K::Tab) => { /* tab completion */ }
              (Command, K::BackTab) => { /* tab completion? */ }
              (Command, K::Esc) => { self.clear_input(); self.input_mode = InputMode::Menu; }
              (Command, K::Backspace) => { self.input_backspace_char(event.modifiers == M::ALT); }
              (Command, K::Delete) => { self.input_delete_char(); }
              (Command, K::Left) => { self.input_cursor_left(); }
              (Command, K::Right) => { self.input_cursor_right(); }
              (Command, K::Home) => { self.input_home(); }
              (Command, K::End) => { self.input_end(); }
              (Command, K::Char(c)) => { self.input_new_char(c); }
              (Command, K::Enter) => {
                let mut err: Option<String> = None;
                let input_string = self.input_string.clone();
                let mut split = input_string.split_whitespace().peekable();
                let mut output_lines = Vec::new();
                while let Some(command) = split.next() {
                  match command {
                    "exit" | "quit" => {
                      exit = true;
                    }
                    "user" => {
                      if let Some(user) = split.next() {
                        if let Ok(user) = u64::from_str_radix(user, 16) {
                          sim_channel_tx.send(SimCommand::SetUser(user))?;
                          self.sim_state.active_user = user;
                        } else {
                          err = Some(format!("Invalid user. Expected a number but got: {}", user));
                        }
                      }
                    }
                    "debug" => {
                      self.sim_state.debug_mode = !self.sim_state.debug_mode;
                      sim_channel_tx.send(SimCommand::Debug(self.sim_state.debug_mode))?;
                    }
                    "summon" => {
                      sim_channel_tx.send(SimCommand::Summon)?;
                    }
                    "r" | "run" | "starr" | "resume" => {
                      sim_channel_tx.send(SimCommand::Run)?;
                    }
                    "speed" => {
                      if let Some(speed) = split.peek() {
                        if let Ok(speed) = speed.parse::<usize>() {
                          sim_channel_tx.send(SimCommand::TickRate(speed))?;
                          split.next();
                        } else {
                          err = Some(format!("Invalid speed. Expected a number but got: {}", speed));
                        }
                      } else {
                        err = Some(S!("Invalid speed. Expected a number."));
                      }
                    }
                    "g" | "go" | "goto" => {
                      match self.view_mode {
                        ViewMode::Code => {
                          if let Some(addr) = split.peek() {
                            if let Ok(addr) = u16::from_str_radix(addr, 16) {
                              // self.code_scroll = addr as usize;
                              self.code_offset = addr as usize;
                              split.next();
                            } else {
                              err = Some(format!("Invalid address. Expected a number but got: {}", addr));
                            }
                          }
                        }
                        ViewMode::Memory => {
                          if let Some(addr) = split.peek() {
                            if let Ok(addr) = u16::from_str_radix(addr, 16) {
                              self.memory_scroll = addr as usize;
                              split.next();
                            } else {
                              err = Some(format!("Invalid address. Expected a number but got: {}", addr));
                            }
                          }
                        }
                        ViewMode::Log => {

                        }
                      }
                    }
                    "load" => {
                      let file_path = if let Some(path) = split.next() {
                        PathBuf::from(path)
                      } else {
                        err = Some(S!("No file path provided."));
                        continue;
                      };

                      if file_path.exists() {
                        let bin = load_wavevm_bin(file_path.to_str().unwrap());
                        match bin {
                          Ok(bin) => {
                            let mlen = bin.mem.len();
                            let clen = bin.code.len();
                            sim_channel_tx.send(SimCommand::WriteAll(0, bin.mem))?;
                            sim_channel_tx.send(SimCommand::WriteAll(0x40, bin.code))?;
                            self.print_plain(format!("Loaded {} bytes of memory and {} bytes of code from {}", mlen, clen, file_path.display()));
                          }
                          Err(msg) => {
                            err = Some(format!("Failed to load file: {}", msg));
                          }
                        }
                      } else {
                        err = Some(format!("File not found: {}", file_path.display()));
                      }
                    }
                    "reset" | "clear" => {
                      sim_channel_tx.send(SimCommand::Reset)?;
                    }
                    "restart" => {
                      sim_channel_tx.send(SimCommand::Restart)?;
                    }
                    "write" => {
                      sim_channel_tx.send(SimCommand::WriteCommand(self.input_string.clone()))?;
                    }
                    "code" => {
                      sim_channel_tx.send(SimCommand::CodeCommand(self.input_string.clone()))?;
                    }
                    "bp" | "breakpoint" => {
                      if let Some(addr) = split.peek() {
                        if let Ok(addr) = u16::from_str_radix(addr, 16) {
                          // Check if breakpoint exists
                          if let Some(i) = self.breakpoints.iter().position(|&x| x == addr) {
                            self.breakpoints.remove(i);
                            split.next();
                          } else {
                            self.breakpoints.push(addr);
                            split.next();
                          }

                          sim_channel_tx.send(SimCommand::Breakpoints(self.breakpoints.clone()))?;
                        } else {
                          // err = Some(format!("Invalid address. Expected a number but got: {}", addr));
                          info!("Invalid address. Expected a number but got: {}", addr);
                        }
                      }
                    }
                    "watch" => {
                      if let Some(sub) = split.peek() {
                        match sub {
                          &"a" | &"add" => {

                          },
                          &"r" | &"rem" | &"remove" => {

                          }
                          &"l" | &"list" => {
                            for (addr, size, name) in self.watch_addr.iter() {
                              if let Some(name) = name {
                                output_lines.push(format!("{:04x} {:02x} {}", addr, size, name));
                              } else {
                                output_lines.push(format!("{:04x} {:02x}", addr, size));
                              }
                            }
                          }
                          _ => {
                            info!("Invalid watch command. Expected 'add' or 'remove' but got: {}", sub);
                          }
                        }
                      }
                      // if let Some(addr) = split.peek() {
                      //   if let Ok(addr) = u16::from_str_radix(addr, 16) {
                      //     self.watch_addr = addr;
                      //     split.next();
                      //   } else {
                      //     // err = Some(format!("Invalid address. Expected a number but got: {}", addr));
                      //     info!("Invalid address. Expected a number but got: {}", addr);
                      //   }
                      // }
                    }
                    "peek" => {
                      if let Some(addr) = split.peek() {
                        if let Ok(addr) = u16::from_str_radix(addr, 16) {
                          sim_channel_tx.send(SimCommand::Read(addr))?;
                          split.next();
                        } else {
                          // err = Some(format!("Invalid address. Expected a number but got: {}", addr));
                          info!("Invalid address. Expected a number but got: {}", addr);
                        }
                      }
                    }
                    "poke" => {
                      if let Some(peek) = split.peek() {
                        let register = match *peek {
                          "c0" => Some(0),  "c1" => Some(1),
                          "c2" => Some(2),  "c3" => Some(3),
                          "c4" => Some(4),  "c5" => Some(5),
                          "c6" => Some(6),  "c7" => Some(7),
                          "r0" => Some(8),  "r1" => Some(9),
                          "r2" => Some(10), "r3" => Some(11),
                          "r4" => Some(12), "r5" => Some(13),
                          "r6" => Some(14), "r7" | "ri" => Some(15),
                          _ => None,
                        };
                        let addr;
                        if let Some(reg) = register {
                          addr = reg * 4;
                        } else if let Ok(a) = u16::from_str_radix(peek, 16) {
                          addr = a;
                        } else {
                          // err = Some(format!("Invalid address. Expected a number but got: {}", peek));
                          info!("Invalid address. Expected a number but got: {}", peek);
                          continue;
                        }
                        split.next(); // Consume the address
                        let mut offset = 0;
                        while let Some(value) = split.peek() {
                          if value.len() > 4 {
                            let mut i = 0;
                            let mut buffer = 0u16;
                            for c in value.chars() {
                              match c {
                                '0'..='9' |
                                'a'..='f' |
                                'A'..='F' => {
                                  buffer = (buffer << 4) + c.to_digit(16).unwrap() as u16;
                                  i += 1;
                                  if i >= 4 {
                                    sim_channel_tx.send(SimCommand::Write(addr + offset, buffer))?;
                                    offset += 1;
                                    buffer = 0;
                                    i = 0;
                                  }
                                },
                                _ => {
                                  continue;
                                }
                              }
                            }

                            if i > 0 {
                              // Leftover nibbles
                              sim_channel_tx.send(SimCommand::Write(addr + offset, buffer))?;
                              offset += 1;
                            }
                          } else {
                            if let Ok(value) = u16::from_str_radix(value, 16) {
                              sim_channel_tx.send(SimCommand::Write(addr + offset, value))?;
                              offset += 1;
                            } else {
                              // err = Some(format!("Invalid value. Expected a number but got: {}", value));
                              info!("Invalid value. Expected a number but got: {}", value);
                              break;
                            }
                          }

                          split.next();
                        }
                      }
                    }
                    "s" | "step" | "tick" => {
                      // sim_channel_tx.send(SimCommand::Run)?;
                      self.sim_state.running = false;
                      self.sim_state.debug_mode = true;
                      sim_channel_tx.send(SimCommand::Step)?;
                    }
                    _ => ()//err = Some(format!("Unknown command: {}", command)),
                  }
                }

                if let Some(err) = err {
                  self.print_plain(format!("Error: {}", err));
                } else {
                  self.print_plain(self.input_string.clone());
                  for line in output_lines {
                    self.print_plain(line);
                  }
                }

                self.input_string.clear();
                self.input_cursor = 0;
                self.input_mode = InputMode::Menu;
              }
              _ => ()
            }
          }
          event::Event::Mouse(mouse) => {
            match mouse.kind {
              event::MouseEventKind::Moved => {
                self.mouse_pos = Some(Position { x: mouse.column, y: mouse.row });
              }
              event::MouseEventKind::Drag(_button) => {
                self.mouse_pos = Some(Position { x: mouse.column, y: mouse.row });
                self.input_drop(mouse_down.x, mouse_down.y, mouse.column, mouse.row);
              }
              event::MouseEventKind::ScrollDown => self.input_scroll(-1),
              event::MouseEventKind::ScrollUp => self.input_scroll(1),
              event::MouseEventKind::Up(button) => {
                match button {
                  event::MouseButton::Left => {
                    if mouse_down.x == mouse.column && mouse_down.y == mouse.row {
                      self.input_click(mouse.column, mouse.row);
                    } else {
                      self.input_drop(mouse_down.x, mouse_down.y, mouse.column, mouse.row);
                    }
                  }
                  _ => ()
                }
              },
              event::MouseEventKind::Down(button) => {
                match button {
                  event::MouseButton::Left => {
                    mouse_down.x = mouse.column;
                    mouse_down.y = mouse.row;
                  }
                  _ => ()
                }
              }
              _ => {
                // self.print_plain(format!("Mouse event: {:?}", mouse));
              }
            }
          }
          _ => ()
        }
      }

      if exit {
        break Ok(());
      }

      while let Ok(output) = sim_output_rx.try_recv() {
        match output {
          SimOutput::MemoryValue(_user, addr, val) => {
            if self.sim_state.active_user == _user {
              self.sim_state.memory[addr as usize] = val;
            }
          }
          SimOutput::MemoryValues(user, addr, vals) => {
            if self.sim_state.active_user == user {
              for (i, val) in vals.iter().enumerate() {
                self.sim_state.memory[addr as usize + i] = *val;
              }
            }
          }
          SimOutput::ChangeUser(user) => {
            self.print_plain(format!("Active user changed to {}", user));
          }
          SimOutput::Error(err) => {
            self.print_plain(format!("Error: {}", err));
          }
          SimOutput::SimState(user, state) => {
            if user == self.sim_state.active_user {
              self.sim_state.running = state.running;
              self.sim_state.debug_mode = state.debug_mode;
              self.sim_state.sleep = state.sleep;
              self.sim_state.defer = state.defer;
              if self.sim_state.debug_mode && self.sim_state.sleep == 0 && self.sim_state.running {
                let a = self.sim_state.memory[0x3c] & 0x1fff;
                let i = self.sim_state.memory[a as usize];
                let o = Opcode::parse(i);
                self.printc(vec![
                  (S!("U"), Color::White),
                  (format!("{}", user), Color::LightBlue),
                  (S!(": "), Color::White),
                  (format!("@{:04x}[{:04x}] {}", a, i, o), Color::White),
                ]);
              }
              if let Some(ref reason) = state.halt_reason {
                self.print_plain(format!("User {} halted: {}", user, reason));
              }
            }
          }
          SimOutput::ShipState(_user, ship) => {
            // debug!("Ship state: {:?}", ship);
            // self.ship = ship;
            self.ship.phy.pos.x = ship.phy.pos.x;
            self.ship.phy.pos.y = ship.phy.pos.y;
            self.ship.phy.heading = ship.phy.heading;
            self.ship.flight.color = ship.flight.color;

            // ship_state_tx.send((ship, self.ui_regions.full)).unwrap();
          }
        }
      }
    }
  }

  fn print<'a>(&'a mut self, text: Vec<ColoredString>) {
    self.log_strings.push(text);
    if self.log_strings.len() > 200 {
      self.log_strings.remove(0);
    }
    self.log_position = self.log_position.saturating_sub(1).min(self.log_strings.len());
  }

  fn printc(&mut self, strings: Vec<(String, Color)>) {
    let colored_strings = strings.into_iter()
      .map(|(text, color)| ColoredString { text, color })
      .collect::<Vec<_>>();
    self.print(colored_strings);
  }

  fn print_plain<'a>(&'a mut self, text: String) {
    let colored_text = ColoredString {
      text,
      color: Color::White,
    };
    self.print(vec![colored_text]);
  }

  fn draw(&mut self, frame: &mut Frame) {
    self.ui_regions = generate_regions(frame);

    if !self.ui_regions.valid {
      frame.render_widget(Paragraph::new("Terminal too small!")
        .centered()
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Red))
      , frame.area());
      return;
    }

    match self.view_mode {
      ViewMode::Log => {
        self.draw_log_view(frame, self.ui_regions.main);
      }
      ViewMode::Memory => {
        self.draw_memory_view(frame, self.ui_regions.main);
      }
      ViewMode::Code => {
        self.draw_code_view(frame, self.ui_regions.main);
      }
    }

    self.draw_status_box(frame);
    self.draw_registers_box(frame);
    self.draw_watch_boxes(frame);
    // self.draw_memory_box(frame);

    match self.input_mode {
      InputMode::Menu => {
        self.draw_keymap(frame);
      }
      InputMode::Command => {
        self.draw_input_box(frame);
      }
    }

    self.draw_ship(frame);

    self.mouse_clicks.clear();
  }

  fn draw_ship(&mut self, frame: &mut Frame) {
    // debug!("Ship: {:?}", self.ship);
    let ship = &self.ship;

    let h = (ship.phy.heading * 8f32 + (1f32/16f32)).floor() as u8;
    let h = h % 8;
    let triangle = match h {
      0 => "⇑",
      1 => "⇗",
      2 => "⇒",
      3 => "⇘",
      4 => "⇓",
      5 => "⇙",
      6 => "⇐",
      7 => "⇖",
      _ => "⇑",
    };
    let w = frame.area().width as f32;
    let h = frame.area().height as f32;
    let x = (ship.phy.pos.x / 1920f32) * (w - 1f32);
    let y = (ship.phy.pos.y / 1080f32) * (h - 1f32);
    let color = ship.flight.color;
    let color = color_from_value(color);

    let x = (x as u16).clamp(0, w as u16 - 1);
    let y = (y as u16).clamp(0, h as u16 - 1);

    // debug!("Ship: {:?} {} {} {} {}", ship.x, ship.y, x, y, flight.current_compass);

    frame.buffer_mut().set_string(x, y, triangle, Style::default().fg(color));

    // if self.ship_image.is_some() {
    //   let mut image = self.ship_image.as_mut().unwrap();
    //   let image = Image::new(&mut image);
    //   frame.render_widget(image, Rect::new(x, y, 4, 2));
    // }
    // let mut ship_image = self.render_ship_image();

    // let image = Image::new(&mut ship_image);

    // frame.render_widget(image, Rect::new(x, y, 2, 1));
  }

  fn draw_code_view(&mut self, frame: &mut Frame, rect: Rect) {
    let m = self.sim_state.memory[self.code_offset..(self.code_offset+0xc0)].iter();
    let pc = self.sim_state.memory[0x3c] as usize;
    let mut lines = Vec::new();
    // let mut prev_was_pcinc = false;
    // let mut prev_dst = 0;
    let mut loading = 0;
    for (i, &val) in m.enumerate() {
      let mut spans = Vec::new();
      let addr = self.code_offset as u16 + i as u16;

      let mouse_over = self.mouse_pos
        .map(|pos| {
          pos.y == (i.saturating_sub(self.code_scroll)) as u16 + rect.y
          && rect.contains(pos)
        })
        .unwrap_or(false);

      if mouse_over && self.mouse_clicks.len() > 0 {
        if let Some(click) = self.mouse_clicks.last() {
          // let x = click.0 as u16;
          let y = click.y as u16;

          if y == (i.saturating_sub(self.code_scroll)) as u16 + rect.y {
            self.actions.push(AppActions::Breakpoint(addr));
            self.mouse_clicks.pop();
          }
        }
      }

      if self.breakpoints.contains(&addr) {
        spans.push("●".to_string().red());
      } else {
        if mouse_over {
          spans.push("●".to_string().dark_gray());
        } else {
          spans.push(" ".to_string().white());
        }
      }
      if i + self.code_offset == pc {
        spans.push(">".to_string().green());
      } else {
        spans.push(" ".to_string().white());
      }
      spans.push("[".to_string().dark_gray());
      // spans.push("00".to_string().fg(Color::Rgb(64, 64, 64)));
      spans.push(format!("{:02x}", addr).white());
      spans.push(S!("] ").dark_gray());
      spans.push(format!("{:04x}", val).fg(color_from_value(val)));
      spans.push(S!(": ").dark_gray());


      let opcode = Opcode::parse(val);
      let load_literal = match opcode {
        Opcode::LoadInc(src, _, opt) |
        Opcode::StoreInc(src, _, opt) |
        Opcode::GatherInc(src, _, opt) |
        Opcode::ScatterInc(src, _, opt) => {
          if RegIndex::from(src as u8) == RegIndex::Ri {
            loading = (opt & 0b11) + 2;
            true
          } else {
            false
          }
        },
        _ => false
      };

      let likely_literal = loading > 0;
      loading = loading.saturating_sub(1);

      let src_val1 = self.sim_state.memory[(addr as usize + 1) % MEM_SHARED_SIZE_U];
      let src_val2 = self.sim_state.memory[(addr as usize + 2) % MEM_SHARED_SIZE_U];
      let src_val3 = self.sim_state.memory[(addr as usize + 3) % MEM_SHARED_SIZE_U];
      let src_val4 = self.sim_state.memory[(addr as usize + 4) % MEM_SHARED_SIZE_U];

      let src_val = match opcode {
        Opcode::LoadInc(_, _, opt) |
        Opcode::StoreInc(_, _, opt) |
        Opcode::GatherInc(_, _, opt) |
        Opcode::ScatterInc(_, _, opt) => {
          match opt {
            0x0 => format!("{:04x}", src_val1),
            0x1 => format!("{:04x}, {:04x}", src_val1, src_val2),
            0x2 => format!("{:04x}, {:04x}, {:04x}", src_val1, src_val2, src_val3),
            _   => format!("{:04x}, {:04x}, {:04x}, {:04x}", src_val1, src_val2, src_val3, src_val4),
          }
        },
        _ => format!("{:04x}", src_val1),
      };

      if load_literal {
        spans.push(opcode.to_string().gray());
        spans.push(" <- ".green());
        spans.push(src_val.blue());
      } else {
        if likely_literal {
          spans.push(format!("{} ", opcode).fg(Color::Rgb(64, 64, 64)));
        } else {
          spans.push(format!("{} ", opcode).white());
        }
      }

      lines.push(Line::from(spans));
    }

    let start_line = self.code_scroll;
    let end_line = start_line + rect.height as usize;

    let lines = lines.iter()
      .skip(start_line)
      .take(end_line - start_line)
      .cloned()
      .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(lines.to_vec()), rect);

    let max = 0x60;
    let scroll_start = self.code_scroll as i32 - self.code_offset as i32;
    let scroll_end = scroll_start + rect.height as i32 - 1 - self.code_offset as i32;
    self.draw_scrollbar(frame, true, max as i16, scroll_start as i16, scroll_end as i16);
  }

  fn draw_memory_view(&mut self, frame: &mut Frame, rect: Rect) {
    let mut lines = Vec::new();

    let start = self.memory_scroll;
    let end = start + rect.height as usize;

    let block_width = 8;
    let block_x = 4 + 4 + 2 + 2;

    let mut modules: [Option<Module>; 8] = [None; 8];

    for addr in 0x318..0x320 {
      let val = self.sim_state.memory[addr];
      modules[addr - 0x318] = Module::type_from_id(val, addr as u16 - 0x318);
    }

    let mut loading: u8 = 0;

    for addr in start..end {
      if addr >= MEM_SHARED_SIZE_U {
        break;
      }
      let val = self.sim_state.memory[addr];
      let modulo = addr % 4;
      let pre_char = match modulo {
        0 => S!("╭").dark_gray(),
        3 => S!("╰").dark_gray(),
        _ => S!("│").dark_gray(),
      };
      let group_char = match modulo {
        0 => format!("{}──╮ ", "─".repeat(block_width)).dark_gray(),
        3 => format!("{}──╯ ", "─".repeat(block_width)).dark_gray(),
        _ => format!("{}  │ ", " ".repeat(block_width)).dark_gray(),
      };

      let opcode = Opcode::parse(val);
      let load_literal = match opcode {
        Opcode::LoadInc(src, _, opt) |
        Opcode::StoreInc(src, _, opt) |
        Opcode::GatherInc(src, _, opt) |
        Opcode::ScatterInc(src, _, opt) => {
          if RegIndex::from(src as u8) == RegIndex::Ri {
            loading = (opt & 0b11) + 2;
            true
          } else {
            false
          }
        },
        _ => false
      };

      let likely_literal = loading > 0;
      loading = loading.saturating_sub(1);

      let src_val1 = self.sim_state.memory[(addr + 1) % MEM_SHARED_SIZE_U];
      let src_val2 = self.sim_state.memory[(addr + 2) % MEM_SHARED_SIZE_U];
      let src_val3 = self.sim_state.memory[(addr + 3) % MEM_SHARED_SIZE_U];
      let src_val4 = self.sim_state.memory[(addr + 4) % MEM_SHARED_SIZE_U];

      let src_val = match opcode {
        Opcode::LoadInc(_, _, opt) |
        Opcode::StoreInc(_, _, opt) |
        Opcode::GatherInc(_, _, opt) |
        Opcode::ScatterInc(_, _, opt) => {
          match opt {
            0x0 => format!("{:04x}", src_val1),
            0x1 => format!("{:04x}, {:04x}", src_val1, src_val2),
            0x2 => format!("{:04x}, {:04x}, {:04x}", src_val1, src_val2, src_val3),
            _   => format!("{:04x}, {:04x}, {:04x}, {:04x}", src_val1, src_val2, src_val3, src_val4),
          }
        },
        _ => format!("{:04x}", src_val1),
      };

      let module;

      if let Some(slot) = Module::addr_to_slot(addr as u16) {
        match modules[slot] {
          Some(m) => module = Some(m),
          None => module = None,
        }
      } else {
        module = None;
      }

      let desc = match addr {
        0x40..0x100 => {
          match (load_literal, likely_literal) {
            (true, _) => vec!(opcode.to_string().gray(), " <- ".green(), src_val.blue()),
            (_, true) => vec!(opcode.to_string().fg(Color::Rgb(64, 64, 64))),
            _ => vec!(format!("{}", opcode).gray()),
          }
        },
        _ => {
          if let Some(m) = module && let Some((_, desc)) = m.module_register_info(addr) {
            vec!(desc)
          } else {
            vec![format!("{}", opcode).fg(Color::Rgb(64, 64, 64))]
          }
        }
      };

      let mut spans = vec![
        format!("{:04x}", addr).dark_gray(),
        format!(": {}", pre_char).dark_gray(),
        format!("{:04x}", self.sim_state.memory[addr]).fg(color_from_value(self.sim_state.memory[addr])),
        format!("{}", group_char).dark_gray(),
      ];
      for l in desc { spans.push(l); }
      lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), rect);

    for addr in start..end {
      let modulo = addr % 4;
      if modulo == 0 {
        let r = Rect::new(
          rect.x + block_x as u16,
          rect.y + addr as u16 - self.memory_scroll as u16,
          block_width as u16,
          1,
        );

        let word_name = match addr {
          0x00..0x40 => format!("{:?}", RegIndex::from(addr as u8 / 4)),
          _ => S!(""),
        };

        frame.render_widget(Paragraph::new(Line::from(vec![
          S!(word_name).white()
        ]).centered()), r);
      }

      let addr_name = Module::addr_to_slot(addr as u16)
        .and_then(|slot| modules[slot])
        .and_then(|m| m.module_register_info(addr))
        .and_then(|(name, _)| Some(name));

      if let Some(name) = addr_name {
        let r = Rect::new(
          rect.x + block_x as u16,
          rect.y + addr as u16 - self.memory_scroll as u16,
          block_width as u16,
          1,
        );
        frame.render_widget(Paragraph::new(name), r);
      }
    }

    let max = (MEM_SHARED_SIZE_U as i32 - 1) - self.ui_regions.memory.height as i32;
    let scroll_start = self.memory_scroll as i32;
    let scroll_end = scroll_start + self.ui_regions.memory.height as i32 - 1;

    self.draw_scrollbar(frame, true, max as i16, scroll_start as i16, scroll_end as i16);
  }

  fn draw_log_view(&mut self, frame: &mut Frame, rect: Rect) {
    let max_lines = rect.height as usize;
    let offset: isize = max_lines as isize - self.log_strings.len() as isize;

    // Print all the scrollback lines
    for (i, line) in self.log_strings.iter().enumerate() {
      let y: isize = rect.y as isize + i as isize + self.log_position as isize + offset;
      if y < 0 || y > max_lines as isize {
        continue;
      }
      let y = y as u16;
      let mut x = rect.x;
      for colored_string in line {
        let text = Paragraph::new(colored_string.text.clone())
          .style(Style::default().fg(colored_string.color));
        frame.render_widget(text, Rect::new(x, y, colored_string.text.len() as u16, 1));
        x += colored_string.text.len() as u16;
      }
    }

    let max_lines = rect.height as usize;
    let scroll_end = (self.log_strings.len() - self.log_position) as isize;
    let scroll_start = scroll_end - max_lines as isize;
    self.draw_scrollbar(frame, true, self.log_strings.len() as i16, scroll_start as i16, scroll_end as i16);
  }

  fn draw_scrollbar(&mut self, frame: &mut Frame, vertical: bool, length: i16, start: i16, end: i16) {
    frame.render_widget(CustomScrollbar {
      vertical,
      start,
      end,
      length,
      color: Color::White,
      track_color: Color::DarkGray,
    }, self.ui_regions.scrollbar);
  }

  fn draw_status_box(&mut self, frame: &mut Frame) {
    let user_no = format!("User: {:0x}", self.sim_state.active_user);
    let run_state = match (self.sim_state.running, self.sim_state.debug_mode) {
      (true, true) => "DEBUG RUN".fg(Color::LightYellow),
      (true, false) => "RUN".fg(Color::LightGreen),
      (false, true) => "DEBUG HALT".fg(Color::LightRed),
      (false, false) => "HALT".fg(Color::LightRed),
    };
    let status_block = Block::bordered()
      .title_top(Line::from("Status").left_aligned())
      .title_top(Line::from(user_no).right_aligned())
      .title_top(Line::from(run_state).centered())
      .style(Style::default().fg(Color::White))
      .border_type(BorderType::Rounded);
    frame.render_widget(Clear, self.ui_regions.status);
    frame.render_widget(status_block, self.ui_regions.status);

    let pc = self.sim_state.memory[0x3c] & 0x1fff;
    let inst = self.sim_state.memory[pc as usize];
    let opcode = Opcode::parse(inst);

    // debug!("{opcode}");

    let p = Paragraph::new(format!("@{:04x}[{:04x}]: {}", pc, inst, opcode))
      .style(Style::default().fg(Color::White));
    let w = self.ui_regions.status.width - 4;
    let rect = Rect::new(self.ui_regions.status.x + 2, self.ui_regions.status.y + 1, w, 2);
    frame.render_widget(p, rect);

    if self.sim_state.sleep > 0 {
      frame.render_widget(Paragraph::new(format!("Sleep: {}", self.sim_state.sleep))
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Right), rect);
    }
    // frame.render_widget(Paragraph::new(user_no_padded)
    //   .style(Style::default().fg(Color::DarkGray))
    // , rect_within(Rect::new(2, 1, 22, 1), self.ui_regions.status));
  }

  fn draw_registers_box(&mut self, frame: &mut Frame) {
    let registers_block = Block::bordered()
      .title_top("Registers")
      .style(Style::default().fg(Color::White))
      .border_type(BorderType::Rounded);
    frame.render_widget(Clear, self.ui_regions.registers);
    frame.render_widget(registers_block, self.ui_regions.registers);

    for reg in 0..16 {
      let col = reg % 2;
      // c0: 0000 0000 0000 0000
      let rw = 4 + 1 + 4 * (4 + 1);
      let x = self.ui_regions.registers.x + 2 + col * (rw + 1);
      let y = self.ui_regions.registers.y + 1 + reg / 2;

      let (reg_name, color) = match reg {
        0..=7 => (format!("  c{:1}:", reg), Color::LightBlue),
        8..=14 => (format!("  r{:1}:", reg - 8), Color::LightBlue),
        15 => ("  ri:".to_string(), Color::LightGreen),
        _ => unreachable!(),
      };
      render_string(frame, reg_name, x, y, 6, Some(color));

      for i in 0..4 {
        let reg_value = self.sim_state.memory[(reg * 4 + i) as usize];
        render_hex(frame, reg_value, x + 6 + i * 5, y, Some(color_from_value(reg_value)));
      }
    }
  }

  fn draw_watch_boxes(&mut self, frame: &mut Frame) {
    let x = self.ui_regions.memory.x;
    let mut y = self.ui_regions.memory.y;
    let w = self.ui_regions.memory.width;
    for (addr, size, str) in self.watch_addr.clone() {
      let start = addr as usize;
      let end = start + (size * 4) as usize;
      let bh = (((end as f32 - start as f32) / 4f32).ceil() as u16 / 2).max(1) + 2;
      let rect = Rect::new(x, y, w, bh);
      self.draw_watch_box(frame, rect, start, end, str);
      y += bh;
    }
  }

  fn draw_watch_box(&mut self, frame: &mut Frame, rect: Rect, start: usize, end: usize, str: Option<String>) {
    let rect = rect.intersection(frame.area());

    let mut watch_block = Block::bordered()
      .title_top("Watch")
      .style(Style::default().fg(Color::White))
      .border_type(BorderType::Rounded);

    if let Some(str) = str {
      watch_block = watch_block.title_top(Line::from(str).light_yellow().centered());
    }
    frame.render_widget(Clear, rect);
    frame.render_widget(watch_block, rect);

    // let start = self.watch_addr as usize;
    let end = (end as f32 / 4f32).ceil() as usize * 4;
    let end = if end <= start { start + 4 } else { end };

    for addr in (start..end).step_by(4) {
      let i = (addr - start) / 4;
      let x = rect.x + 2 + (i % 2 * 26) as u16;
      let y = rect.y + 1 + (i / 2) as u16;
      if y >= rect.height + rect.y - 1 {
        break;
      }

      let mx = self.sim_state.memory[addr];
      let my = self.sim_state.memory[addr + 1];
      let mz = self.sim_state.memory[addr + 2];
      let mw = self.sim_state.memory[addr + 3];

      render_hex(frame, addr as u16, x, y, Some(Color::Gray));
      render_string(frame, ":".to_owned(), x + 4, y, 1, Some(Color::White));
      render_hex(frame, mx, x + 6, y, Some(color_from_value(mx)));
      render_hex(frame, my, x + 11, y, Some(color_from_value(my)));
      render_hex(frame, mz, x + 16, y, Some(color_from_value(mz)));
      render_hex(frame, mw, x + 21, y, Some(color_from_value(mw)));
    }
  }

  fn draw_input_box(&mut self, frame: &mut Frame) {
    let style = Style::default()
      .fg(Color::White)
      .add_modifier(Modifier::ITALIC);
    let input_box = Block::bordered()
      .title_top("Command")
      .style(style)
      .border_type(BorderType::Rounded)
      .padding(Padding::new(1,1,1,1));
    let input_text = Paragraph::new(self.input_string.as_str());
    frame.render_widget(Clear, self.ui_regions.input);
    frame.render_widget(input_box, self.ui_regions.input);
    frame.set_cursor_position(Position::new(
      self.ui_regions.input.x + self.input_cursor as u16 + 2,
      self.ui_regions.input.y + 1,
    ));
    frame.render_widget(input_text, rect_within(Rect::new(2, 1, self.ui_regions.input.width - 4, 1), self.ui_regions.input));
  }

  fn draw_keymap(&mut self, frame: &mut Frame) {
    let block = Block::bordered()
      .title_top("Keymap")
      .style(Style::default().fg(Color::White))
      .border_style(Style::default().fg(Color::DarkGray))
      .border_type(BorderType::Rounded);

    frame.render_widget(Clear, self.ui_regions.input);
    frame.render_widget(block, self.ui_regions.input);

    let line = Line::from(vec![
      "View (".white(),
      format!("{:?}", self.view_mode).light_cyan(),
      "): ".white(),
      "[Tab] ".light_blue(),
      "Command: ".white(),
      "[Space] ".light_blue(),
      "Run: ".fg(match (self.sim_state.running, self.sim_state.debug_mode) {(true, true) => Color::Yellow, (true, false) => Color::Green, _ => Color::White}),
      "[r] ".light_blue(),
      "Halt: ".fg(if self.sim_state.running { Color::White } else { Color::Red }),
      "[R] ".light_blue(),
      "Step: ".white(),
      "[s] ".light_blue(),
      "Debug: ".fg(if self.sim_state.debug_mode { Color::Green } else { Color::White }),
      "[d] ".light_blue(),
      "Exit: ".white(),
      "[^C] ".light_blue(),
    ]);

    frame.render_widget(line, self.ui_regions.input.inner(Margin::new(2, 1)));
  }

  fn input_new_char(&mut self, c: char) {
    let byte_index = self.input_string.char_indices().map(|(i, _)| i).nth(self.input_cursor).unwrap_or(self.input_string.len());
    self.input_string.insert(byte_index, c);
    self.input_cursor += 1;
  }

  fn input_delete_char(&mut self) {
    if self.input_cursor < self.input_string.len() {
      let byte_index = self.input_string.char_indices().map(|(i, _)| i).nth(self.input_cursor).unwrap_or(self.input_string.len());
      let before_del = self.input_string.chars().take(byte_index);
      let after_del = self.input_string.chars().skip(byte_index + 1);
      self.input_string = before_del.chain(after_del).collect();
    }
  }

  fn input_backspace_char(&mut self, alt: bool) {
    if self.input_cursor == 0 {
      return;
    }
    if alt {
      let mut first_whitespace = true;
      while self.input_cursor > 0 && match (self.input_string.chars().nth(self.input_cursor - 1), first_whitespace) {
        (Some(' '), true) => true,
        (Some(_), true) => { first_whitespace = false; true }
        (Some(' '), false) => false,
        (Some(_), false) => true,
        _ => false,
      } {
        self.input_backspace_char(false);
      }
    } else {
      let byte_index = self.input_string.char_indices().map(|(i, _)| i).nth(self.input_cursor).unwrap_or(self.input_string.len());
      let start = self.input_string.char_indices().map(|(i, _)| i).nth(self.input_cursor - 1).unwrap_or(0);
      let end = byte_index;
      let before_del = self.input_string.chars().take(start);
      let after_del = self.input_string.chars().skip(end);
      self.input_string = before_del.chain(after_del).collect();
      self.input_cursor -= 1;
    }
  }

  fn input_cursor_left(&mut self) {
    self.input_cursor = self.input_cursor.saturating_sub(1);
  }

  fn input_cursor_right(&mut self) {
    self.input_cursor = self.input_cursor.saturating_add(1).min(self.input_string.len());
  }

  fn input_home(&mut self) {
    self.input_cursor = 0;
  }

  fn input_end(&mut self) {
    self.input_cursor = self.input_string.len();
  }

  fn clear_input(&mut self) {
    self.input_string.clear();
    self.input_cursor = 0;
  }

  fn input_scroll(&mut self, lines: i32) {
    if let Some(mouse) = self.mouse_pos {
      if self.ui_regions.memory.contains(mouse) {

      } else {
        match self.view_mode {
          ViewMode::Log => {
            let new = self.log_position as i32 + lines;
            let max = self.log_strings.len().saturating_sub(1) as i32;
            self.log_position = new.clamp(0, max) as usize;
            debug!("Scroll state: {} {} {}", self.log_strings.len(), self.log_position, lines);
          }
          ViewMode::Memory => {
            let new = self.memory_scroll as i32 - lines;
            let max = (MEM_SHARED_SIZE_U as i32 - 1) - self.ui_regions.memory.height as i32;
            self.memory_scroll = new.clamp(0, max) as usize;
          }
          ViewMode::Code => {
            let new = self.code_scroll as i32 - lines;
            let max = 0x100 - 0x40;
            self.code_scroll = new.clamp(0, max) as usize;
          }
        }
      }
    }
  }

  fn input_click(&mut self, x: u16, y: u16) {
    self.mouse_clicks.push(Position { x, y });
  }

  // fn input_drag(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
  //   // Handle drag events here if needed
  //   self.mouse_drag = Some((Position { x: x1, y: y1 }, Position { x: x2, y: y2 }));
  // }

  fn input_drop(&mut self, x1: u16, y1: u16, x2: u16, y2: u16) {
    // Handle drop events here if needed
    self.mouse_drops.push((Position { x: x1, y: y1 }, Position { x: x2, y: y2 }));
  }

}
