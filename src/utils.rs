use ratatui::{
  buffer::Buffer, layout::Rect, style::{
    Color,
    Style
  }, widgets::{Paragraph, Widget}, Frame
};

#[macro_export]
macro_rules! S {
    ($s:expr) => {
        $s.to_string()
    };
}

pub struct ColoredString {
  pub text: String,
  pub color: Color,
}

impl ColoredString {
  #[allow(unused)]
  pub fn new(text: String, color: Color) -> Self {
    ColoredString { text, color }
  }
}

#[allow(unused)]
#[derive(Debug, Default)]
pub struct UIRegions {
  pub valid: bool,
  pub full: Rect,
  pub main: Rect,
  pub input: Rect,
  pub sidebar: Rect,
  pub registers: Rect,
  pub memory: Rect,
  pub scrollbar: Rect,
  pub status: Rect,
  // pub watch: Rect,
}

pub fn render_string(frame: &mut Frame, value: String, x: u16, y: u16, w: u16, color: Option<Color>) {
  let color = color.unwrap_or(Color::default());
  let text = Paragraph::new(value)
    .style(Style::default().fg(color));
  if frame.area().width < x + 4 { return; }
  if frame.area().height < y + 1 { return; }
  frame.render_widget(text, Rect::new(x, y, w, 1));
}

pub fn render_hex(frame: &mut Frame, value: u16, x: u16, y: u16, color: Option<Color>) {
  render_string(frame, format!("{:04x}", value), x, y, 4, color);
}

pub fn rect_within(rect: Rect, parent: Rect) -> Rect {
  let x = rect.x + parent.x;
  let y = rect.y + parent.y;
  let w = rect.width;
  let h = rect.height;
  Rect::new(x, y, w, h)
}

#[allow(unused)]
pub enum AttachSide {
  Left,
  Right,
  Top,
  Bottom,
}

pub fn attach_to(rect: Rect, parent: Rect, side: AttachSide) -> Rect {
  use AttachSide::*;
  let x = match side {
    Left => parent.x - rect.width,
    Right => parent.x + parent.width,
    _ => parent.x,
  };
  let y = match side {
    Top => parent.y - rect.height,
    Bottom => parent.y + parent.height,
    _ => parent.y,
  };
  Rect::new(x, y, rect.width, rect.height)
}

pub fn color_from_value(value: u16) -> Color {
  if value == 0 { return Color::Rgb(64, 64, 64); }

  let r = (value >> 11 & 0b11111) as f32 / 31f32;
  let g = (value >> 5 & 0b111111) as f32 / 63f32;
  let b = (value & 0b11111) as f32 / 31f32;
  let r = r * 0.5 + 0.5;
  let g = g * 0.5 + 0.5;
  let b = b * 0.5 + 0.5;
  let r = (r as f32 * 255f32) as u8;
  let g = (g as f32 * 255f32) as u8;
  let b = (b as f32 * 255f32) as u8;
  Color::Rgb(r as u8, g as u8, b as u8)
}

pub fn generate_regions(frame: &mut Frame) -> UIRegions {
  let size = frame.area();
  let (w, h) = (size.width, size.height);

  let sidebar_width = 56;
  let scrollbar_width = 1;
  let main_min_width = 16;

  let input_height = 3;
  let status_height = 4;
  let registers_height = 10;
  let memory_min_height = 10;

  if w < sidebar_width + scrollbar_width + main_min_width ||
     h < status_height + registers_height + memory_min_height {
    return UIRegions {
      valid: false,
      ..Default::default()
    };
  }

  let full = Rect::new(0, 0, w, h);
  let main = Rect::new(0, 0, w - sidebar_width - scrollbar_width, h - input_height);
  let input = Rect::new(0, h - input_height, w, input_height);
  let sidebar = Rect::new(w - sidebar_width, 0, sidebar_width, h - input_height);
  let scrollbar = Rect::new(w - sidebar_width - scrollbar_width, 0, scrollbar_width, sidebar.height);
  let status = rect_within(Rect::new(0, 0, sidebar_width, status_height), sidebar);
  let registers = attach_to(Rect::new(0, 0, sidebar_width, registers_height), status, AttachSide::Bottom);
  // let watch = attach_to(Rect::new(0, 0, sidebar_width, registers_height), registers, AttachSide::Bottom);
  // let my = watch.y + watch.height;
  let my = registers.y + registers.height;
  let memory = Rect::new(sidebar.x, my, sidebar_width, sidebar.height - my);

  UIRegions {
    valid: true,
    full,
    main,
    input,
    sidebar,
    registers,
    memory,
    scrollbar,
    status,
    // watch,
  }
}


pub struct CustomScrollbar {
  pub start: i16, // Start of the view position
  pub end: i16, // End of the view position
  pub length: i16, // Length of the scrollable content
  pub color: Color, // Scrollbar color
  pub track_color: Color, // Track color
  pub vertical: bool, // Whether the scrollbar is vertical or horizontal
}

impl Widget for CustomScrollbar {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let size = if self.vertical { area.height } else { area.width };

    let ratio = size as f32 / self.length as f32;
    let start = (self.start.max(0) as f32 * ratio) as u16;
    let end = (self.end.max(0) as f32 * ratio) as u16;

    // let bar_height = if start == end {
    //   1
    // } else {
    //   (end - start).max(1) as u16
    // };

    // let bar_start = if self.vertical {
    //   area.y + start
    // } else {
    //   area.x + start
    // };

    for y in 0..area.height {
      for x in 0..area.width {
        if (self.vertical && y >= start && y < end) ||
           (!self.vertical && x >= start && x < end) {
          buf.set_string(area.x + x, area.y + y, " ", Style::default().bg(self.color));
        } else {
          buf.set_string(area.x + x, area.y + y, "│", Style::default().fg(self.track_color));
        }
      }
    }

    // let length = self.length;
    // let color = self.color;
    // let track_color = self.track_color;

    // let max = if self.vertical { area.height } else { area.width };
    // if length < max as i16 {
    //   return;
    // }

    // let ratio = max as f32 / length as f32;
    // let mut start = (self.start.max(0) as f32 * ratio) as u16;
    // let mut end = (self.end.max(0) as f32 * ratio) as u16;

    // if start == end {
    //   start = start.min(max - 1);
    //   end = start + 1;
    // }

    // for y in 0..area.height {
    //   for x in 0..area.width {
    //     if (self.vertical && y >= start && y < end) ||
    //        (!self.vertical && x >= start && x < end) {
    //       buf.set_string(area.x + x, area.y + y, " ", Style::default().bg(color));
    //     } else {
    //       buf.set_string(area.x + x, area.y + y, "│", Style::default().fg(track_color));
    //     }
    //   }
    // }
  }
}
