#[derive(Clone, Debug)]
pub struct Cell {
    pub text: char,
    pub style: Style,
}

#[derive(Clone, Debug)]
pub struct Style {
    pub foreground: termion::color::AnsiValue,
    pub background: termion::color::AnsiValue,
    pub bold: bool,
}

impl Style {
    fn update_to(&mut self, other: &Style, output: &mut String) {
        use std::fmt::Write;
        if self.foreground.0 != other.foreground.0 {
            write!(output, "{}", termion::color::Fg(other.foreground)).unwrap();
            self.foreground = other.foreground;
        }
        if self.background.0 != other.background.0 {
            write!(output, "{}", termion::color::Bg(other.background)).unwrap();
            self.background = other.background;
        }
        // Turn bold off
        if self.bold && !other.bold {
            write!(output, "{}", termion::style::NoBold).unwrap();
        }
        // Turn bold on
        if !self.bold && other.bold {
            write!(output, "{}", termion::style::Bold).unwrap();
        }
        self.bold = other.bold;
    }
}

impl PartialEq for Cell {
    fn eq(&self, rhs: &Cell) -> bool {
        self.style.foreground.0 == rhs.style.foreground.0
            && self.style.background.0 == rhs.style.background.0
            && self.style.bold == rhs.style.bold
            && self.text == rhs.text
    }
}

pub struct Screen {
    cells: Vec<Option<Cell>>,
    width: u16,
    height: u16,
}

impl Screen {
    pub fn new() -> Self {
        let (width, height) = termion::terminal_size().unwrap();
        Screen {
            cells: vec![None; (width * height) as usize],
            width,
            height,
        }
    }

    pub fn columns(&self) -> u16 {
        self.width
    }

    pub fn rows(&self) -> u16 {
        self.height
    }

    pub fn get(&self, mut row: u16, mut col: u16) -> &Option<Cell> {
        row -= 1;
        col -= 1;
        &self.cells[(row * self.width + col) as usize]
    }

    pub fn set(&mut self, mut row: u16, mut col: u16, cell: Cell) {
        row -= 1;
        col -= 1;
        self.cells
            .get_mut((row * self.width + col) as usize)
            .map(|c| *c = Some(cell));
    }

    pub fn set_str(
        &mut self,
        row: u16,
        col: u16,
        fg: termion::color::AnsiValue,
        bg: termion::color::AnsiValue,
        bold: bool,
        text: &str,
    ) {
        // TODO: grapheme clusters
        for (c, chr) in text.chars().enumerate() {
            self.set(
                row,
                col + c as u16,
                Cell {
                    style: Style {
                        foreground: fg,
                        background: bg,
                        bold,
                    },
                    text: chr,
                },
            );
        }
    }

    pub fn update_from(&mut self, other: &Screen) -> String {
        use std::fmt::Write;
        use termion::color::{AnsiValue, Bg, Fg};
        use termion::cursor::Goto;
        let mut output = String::new();
        let default_style = Style {
            foreground: AnsiValue::rgb(5, 5, 5),
            background: AnsiValue::rgb(0, 0, 0),
            bold: false,
        };
        let mut current_style = Style {
            foreground: AnsiValue::rgb(5, 5, 5),
            background: AnsiValue::rgb(0, 0, 0),
            bold: false,
        };
        write!(
            output,
            "{}{}{}",
            Fg(current_style.foreground),
            Bg(current_style.background),
            termion::style::Reset
        )
        .unwrap();

        for row in 1..other.height + 1 {
            let mut previous_col = 0;
            let mut wrote_goto_row = false;
            for col in 1..other.width + 1 {
                let prev = self.get(row, col);
                let new = other.get(row, col);
                if prev != new {
                    if !wrote_goto_row {
                        write!(output, "{}", termion::cursor::Goto(1, row)).unwrap();
                        wrote_goto_row = true;
                    }
                    // If we're not writing consecutively across the screen, use a Goto
                    if previous_col != col - 1 {
                        write!(output, "{}", Goto(col, row)).unwrap();
                    }
                    match (prev, new) {
                        (Some(_), None) => {
                            current_style.update_to(&default_style, &mut output);
                            output.push(' ');
                        }
                        (_, Some(new)) => {
                            current_style.update_to(&new.style, &mut output);
                            output.push(new.text);
                        }
                        (None, None) => {}
                    };
                    previous_col = col;
                }
            }
        }

        self.width = other.width;
        self.height = other.height;
        self.cells.clear();
        self.cells.extend_from_slice(&other.cells);
        output
    }
}
