//! This module defines some utility functions to work with the terminal

use std::io::{stdout, Stdout};

use crossterm::{execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, Terminal};
use crate::errors::Result;

/// Convenient alias
pub type Term = Terminal<CrosstermBackend<Stdout>>;

/// Initializes the terminal
pub fn init_term() -> Result<Term> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let term = Terminal::new(CrosstermBackend::new(stdout))?;
    Ok(term)
}

/// Resets the terminal to a useable state by other applications
pub fn reset_term(term: &mut Term) -> Result<()> {
    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    term.show_cursor()?;
    Ok(())
}