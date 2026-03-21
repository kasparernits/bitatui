use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
};

mod api;
mod app;
mod bitcoin;
mod config;
mod error;
mod file;
mod ui;

use app::{App, AppAction};

const VERSION_LABEL: &str = concat!(" bitatui ", env!("CARGO_PKG_VERSION"));
const POLL_INTERVAL: Duration = Duration::from_millis(50);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::initialize()?;

    loop {
        app.update();
        terminal.draw(|f| ui::draw(f, &app, VERSION_LABEL))?;

        if event::poll(POLL_INTERVAL)? {
            if let Event::Key(key) = event::read()? {
                if matches!(app.handle_key_event(key)?, AppAction::Quit) {
                    break;
                }
            }
        }
    }

    Ok(())
}
