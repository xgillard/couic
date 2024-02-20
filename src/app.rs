//! This is where the core of the application is defined

use std::env::current_dir;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::str::FromStr;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use displaythis::Display;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Styled, Stylize};
use ratatui::widgets::{Block, Borders, LineGauge};
use ratatui::Frame;
use regex::Regex;
use tui_textarea::{Input, Key, TextArea};
use tui_prompts::prelude::*;
use lazy_static::*;
use clipboard::*;

use crate::errors::Result;
use crate::term::{init_term, reset_term, Term};

lazy_static!{
    static ref LONG_LINES : Regex = Regex::new(r"[^\n\S]{3,}").unwrap();
}

fn textarea<'a>(lines: Vec<String>, search: &str) -> TextArea<'a> {
    let mut text = TextArea::new(lines);
    text.set_block(Block::new().borders(Borders::all()));

    text.set_style(Style::default()
        .fg(ratatui::style::Color::LightCyan)
    );

    text.set_line_number_style(Style::default()
        .bg(ratatui::style::Color::DarkGray)
        .fg(ratatui::style::Color::White)
    );
    text.set_search_style(Style::default()
        .bg(Color::LightYellow)
        .fg(Color::Red)
    );
    text.set_search_pattern(search).unwrap();
    // les trucs qu'on force a highlight

    text
}


pub struct App<'a> {
    term: Term,
    state: AppState<'a>
}

pub struct AppState<'a> {
    data: Data<'a>,
    view: View
}

pub struct Data<'a> {
    mode: Mode,
    text: TextArea<'a>,
    cwd : TextState<'a>,
    tot : usize,
    curr: TextState<'a>,
    srch: TextState<'a>,
    msg : String,
}

pub struct View;

#[derive(Debug, Clone, Copy, Display)]
pub enum Mode {
    #[display("OPEN-DIR")]
    OpenDir,
    #[display("OPEN-FILE")]
    OpenFile,
    #[display("INPUT")]
    Input,
    #[display("SELECT")]
    Selection,
    #[display("SEARCH")] 
    Search,
    #[display("HISTORY")]
    History,
    #[display("COMMAND")]
    Command,
    #[display("QUIT")]
    Quit
}

impl<'a> App<'a> {
    pub fn new() -> Result<Self> {
        Ok(Self { 
            term: init_term()?, 
            state: AppState::new()
        })
    }

    pub fn run(&mut self) {
        loop {
            let term = &mut self.term;
            let state= &mut self.state;

            if let Err(e) = term.draw(|f| state.ui(f)) {
                state.data.msg = format!("{e}");
            }
            if let Err(e) = state.input() {
                state.data.msg = format!("{e}");
            } else {
                state.data.msg = String::new();
            }

            if matches!(state.mode(), Mode::Quit) {
                break;
            }
        }
    }
}
impl Drop for App<'_> {
    fn drop(&mut self) {
        reset_term(&mut self.term).expect("failed to reset terminal")
    }
}

impl AppState<'_> {
    fn new() -> Self {
        let data = Data::new();
        let view = View::new();

        Self { data, view }
    }
    fn mode(&self) -> Mode {
        self.data.mode
    }
    fn set_mode(&mut self, m: Mode) {
        self.data.mode = m;
    }
    fn ui(&mut self, frame: &mut Frame) {
        let view = &mut self.view;
        let data = &mut self.data;

        view.ui(data, frame)
    }
    fn input(&mut self) -> Result<()> {
        let input = crossterm::event::read()?;

        match self.mode() {
            Mode::OpenDir   => self.open_input(input),
            Mode::OpenFile  => self.curr_input(input),
            Mode::Input     => self.input_input(input),
            Mode::Selection => self.select_input(input),
            Mode::Search    => self.search_input(input),
            Mode::History   => self.history_input(input),
            Mode::Command   => self.command_input(input),
            Mode::Quit      => self.quit_input(input),
        }
    }
    fn open_input(&mut self, input: Event) -> Result<()> {
        match input {
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Esc, ..}) => { 
                self.set_mode(Mode::Command); 
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Enter, ..}) => { 
                let cwd = self.data.cwd.value();
                self.data.tot = read_dir(cwd)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file() )
                    .count();

                self.load(0)?;
                self.set_mode(Mode::Command); 
            },
            Event::Key(event) => { self.data.cwd.handle_key_event(event); },
            _ => { /* ignore */}
        }
        Ok(())
    }
    fn curr_input(&mut self, input: Event) -> Result<()> {
        match input {
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Esc, ..}) => { 
                self.set_mode(Mode::Command); 
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Enter, ..}) => { 
                let curr = self.data.curr.value().parse()?;
                self.load(curr)?;
                self.set_mode(Mode::Command); 
            },
            Event::Key(event) => { self.data.curr.handle_key_event(event); },
            _ => { /* ignore */}
        }
        Ok(())
    }
    fn input_input(&mut self, input: Event) -> Result<()> {
        let input = input.into();
        match input {
            Input { key: Key::Esc, .. } => { self.set_mode(Mode::Command); },
            _ =>  { self.data.text.input(input); }
        }
        Ok(())
    }
    fn select_input(&mut self, input: Event) -> Result<()> {
        if let Some(input) = self.movement(input) {
            match input {
                Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Esc, ..})       => { self.set_mode(Mode::Command); },
                Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('x'), ..}) => { self.data.text.cut(); self.set_mode(Mode::Command); },
                _ => { /* ignore */}
            }
        }
        Ok(())
    }
    fn movement(&mut self, input: Event) -> Option<Event> {
        match input {
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Right, modifiers: KeyModifiers::CONTROL, ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('w'), ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::WordForward); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Left, modifiers: KeyModifiers::CONTROL, ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('b'), ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::WordBack); 
                None
            },
            //
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('u'), modifiers: KeyModifiers::CONTROL, ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::PageUp, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::ParagraphBack); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::PageDown, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::ParagraphForward); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('^'), ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Home, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::Head); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Char('$'), ..}) |
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::End, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::End); 
                None
            },
            //
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Left, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::Back); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Right, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::Forward); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Up, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::Up); 
                None
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Down, ..}) => { 
                self.data.text.move_cursor(tui_textarea::CursorMove::Down); 
                None
            },
            _ => Some(input)
        }
    }

    fn search_input(&mut self, input: Event) -> Result<()> {
        match input {
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Esc, ..}) => { 
                self.set_mode(Mode::Command); 
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Enter, modifiers: KeyModifiers::SHIFT, ..}) => {
                self.data.text.set_search_pattern(self.data.srch.value())?;
                self.data.text.search_back(false);
            },
            Event::Key(KeyEvent{kind: crossterm::event::KeyEventKind::Press, code: KeyCode::Enter, ..}) => {
                self.data.text.set_search_pattern(self.data.srch.value())?;
                self.data.text.search_forward(false);
            }, 
            Event::Key(event) => { self.data.srch.handle_key_event(event); },
            _ => { /* ignore */}
        }
        Ok(())
    }
    fn history_input(&mut self, input: Event) -> Result<()> {
        let input = input.into();
        match input {
            Input { key: Key::Esc, .. }       => { self.set_mode(Mode::Command); },
            Input { key: Key::Char('u'), .. } => { self.data.text.undo(); },
            Input { key: Key::Char('r'), .. } => { self.data.text.redo(); },
            _ => { /* ignore */}
        }
        Ok(())
    }
    fn command_input(&mut self, input: Event) -> Result<()> {
        if let Some(input) = self.movement(input) {
            let input = input.into();
            match input {
                Input { key: Key::Char('q'), .. } => { self.set_mode(Mode::Quit); },
                Input { key: Key::Char('o'), .. } => { self.set_mode(Mode::OpenDir); self.data.cwd.move_end(); },
                Input { key: Key::Char('f'), .. } => { self.set_mode(Mode::OpenFile); self.data.curr.move_end(); },
                Input { key: Key::Char('i'), .. } => { self.set_mode(Mode::Input); },
                Input { key: Key::Char('h'), .. } => { self.set_mode(Mode::History); },
                Input { key: Key::Char('/'), .. } => { self.set_mode(Mode::Search); self.data.srch.move_end(); },
                Input { key: Key::Char('*'), .. } => {
                    let text = self.data.text.lines().join("\n");
                    let mut clipboard: ClipboardContext = ClipboardProvider::new().unwrap();
                    clipboard.set_contents(text).unwrap();
                    self.data.msg = "Filed Copied to Clipboard".to_string();
                },
                //
                Input { key: Key::Char('n'), .. } => { self.next()?; },
                Input { key: Key::Char('p'), .. } => { self.prev()?; },
                Input { key: Key::Char('s'), ctrl: true, .. } => { self.save()?; },
                //
                Input { key: Key::Char('#'), .. } => { self.data.text.insert_str("###"); },
                Input { key: Key::Char('l'), .. } => { self.split_long_lines(); },
                //
                Input { key: Key::Char(' '), .. } => { self.set_mode(Mode::Selection); self.data.text.start_selection(); } 
                _ =>  { /* do nothing */ }
            }
        }
        Ok(())
    }
    fn quit_input(&mut self, _input: Event) -> Result<()> {
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let cwd = PathBuf::from_str(self.data.cwd.value()).unwrap();
        let x: u32 = self.data.curr.value().parse()?;
        let fname = cwd.join(format!("{x:03}.txt"));
        std::fs::remove_file(&fname)?;

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(false)
            .open(fname)?;

        let mut wrt = BufWriter::new(file);
        let text = self.data.text.lines().join("\n");
        wrt.write_all(text.as_bytes())?;
        wrt.flush()?;

        Ok(())
    }

    fn load(&mut self, x: u32) -> Result<()> {
        *self.data.curr.value_mut() = format!("{x:03}");
        let cwd = PathBuf::from_str(self.data.cwd.value()).unwrap();
        let fname = cwd.join(format!("{x:03}.txt"));
        let file = File::open(fname)?;
        let file = BufReader::new(file);

        self.data.text = textarea(file.lines().map(|s| s.unwrap()).collect(), self.data.srch.value());

        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        let curr: u32 = self.data.curr.value().parse()?;
        self.load(curr + 1)
    }
    
    fn prev(&mut self) -> Result<()> {
        let curr: u32 = self.data.curr.value().parse()?;
        self.load(curr - 1)
    }

    fn split_long_lines(&mut self) {
        let text = self.data.text.lines().join("\n");
        let text = LONG_LINES.replace_all(&text, "\n");
        let text = text.lines().map(|s| s.to_owned()).collect();
        self.data.text = textarea(text, self.data.srch.value());
    }
}

impl Data<'_> {
    fn new() -> Self {
        let cwd = current_dir().unwrap_or_default();
        let default_search = r"\d+|f\.|fol|p\.|page|scan";
        Self { 
            mode: Mode::Command,
            text: textarea(vec![], default_search),
            cwd : TextState::new().with_value(cwd.to_string_lossy().to_string()),
            curr: TextState::new().with_value("000"),
            tot : 1,
            srch: TextState::new().with_value(default_search),
            msg : String::new(),
        }
    }
}
impl View {
    fn new() -> Self {
        Self { }
    }

    fn ui(&mut self, data: &mut Data, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ]).split(frame.size());

        let title = Block::new()
            .title_alignment(ratatui::layout::Alignment::Center)
            .title(data.curr.value())
            .add_modifier(Modifier::BOLD)
            .set_style(Style::default().bg(Color::White).fg(Color::Blue));

        let mode = Block::new()
            .title_alignment(ratatui::layout::Alignment::Right)
            .title(format!("{}", data.mode));

        frame.render_widget(title, layout[0]);
        frame.render_widget(data.text.widget(), layout[1]);

        let status_line = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(10)
        ]).split(layout[2]);

        frame.render_widget(mode, status_line[1]);

        match data.mode {
            Mode::OpenDir => {
                TextPrompt::from("Open Directory")
                    .draw(frame, status_line[0], &mut data.cwd);
            },
            Mode::OpenFile => {
                TextPrompt::from("Open File (id only)")
                    .draw(frame, status_line[0], &mut data.curr);
            },
            Mode::Search => {
                TextPrompt::from("Search Pattern")
                    .draw(frame, status_line[0], &mut data.srch);
            },
            _ => {
                if data.msg.is_empty() {
                    let cur: u32 = data.curr.value().parse().unwrap();
                    let ratio = (1 + cur) as f64 / (1 + data.tot) as f64;
                    let progress = LineGauge::default().ratio(ratio);
                    frame.render_widget(progress, status_line[0]);
                } else {
                    let msg = Block::new().title(data.msg.as_str())
                        .style(Style::default().fg(Color::Red));
                    frame.render_widget(msg, status_line[0]);
                }
            }
        }
    }
}