mod pr;
mod utils;

use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pr::PullRequestWidget;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::Stylize,
    text::Line,
};
use std::time::Duration;
use tokio_stream::StreamExt;

use crate::config::Config;

pub async fn run(config: Config) -> Result<()> {
    let terminal = ratatui::init();
    App::new(config).run(terminal).await?;
    ratatui::restore();

    Ok(())
}

#[derive(PartialEq, Eq)]
enum InputMode {
    Normal,
    Searching,
    Help,
}

pub struct App {
    should_quit: bool,
    pull_requests: PullRequestWidget,
    input_mode: InputMode,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 30.0;

    pub fn new(config: Config) -> Self {
        Self {
            should_quit: false,
            pull_requests: PullRequestWidget::new(config),
            input_mode: InputMode::Normal,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.pull_requests.run();
        let period = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        let mut refresh_interval = tokio::time::interval(Duration::from_secs_f32(30_f32));

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| self.draw(frame))?; },
                // Refresh pull requests on interval tick
                _ = refresh_interval.tick() => { self.pull_requests.refresh_pull_requests() },
                Some(Ok(event)) = events.next() => self.handle_event(&event),
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let [title_area, body_area] = vertical.areas(frame.area());
        let title = Line::from("GitMe").centered().bold();
        frame.render_widget(title, title_area);
        frame.render_widget(&self.pull_requests, body_area);
        // Here we need to render the cursor in it's position when we are searching since the api
        // is bind to the frame
        if let Some(cursor_pos) = self.pull_requests.cursor_position() {
            frame.set_cursor_position(cursor_pos);
        }
    }

    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press {
                match self.input_mode {
                    InputMode::Normal => self.handle_normal_input(*key_event),
                    InputMode::Searching => self.handle_searching_input(*key_event, event),
                    InputMode::Help => self.handle_help_input(*key_event),
                }
            }
        }
    }

    fn handle_normal_input(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.pull_requests.scroll_down(),
            KeyCode::Char('k') | KeyCode::Up => self.pull_requests.scroll_up(),
            KeyCode::Char('o') => self.pull_requests.open(),
            KeyCode::Char('r') => self.pull_requests.review(),
            KeyCode::Char('f') => self.pull_requests.refresh_pull_requests(),
            KeyCode::Char('n') => self.pull_requests.next_repository(),
            KeyCode::Char('p') => self.pull_requests.previous_repository(),
            KeyCode::Char('d') => {
                if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.pull_requests.scroll_details_down();
                } else {
                    self.pull_requests.jump_down()
                }
            }
            KeyCode::Char('u') => {
                if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.pull_requests.scroll_details_up();
                } else {
                    self.pull_requests.jump_up();
                }
            }
            KeyCode::Tab => self.pull_requests.next_tab(),
            KeyCode::Char('/') => {
                self.pull_requests.toggle_search();
                self.input_mode = InputMode::Searching;
            }
            KeyCode::Char('?') => {
                self.pull_requests.toggle_help();
                self.input_mode = InputMode::Help;
            }
            _ => {}
        }
    }

    fn handle_searching_input(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        original_event: &Event,
    ) {
        match key_event.code {
            KeyCode::Esc => {
                self.pull_requests.clear_search();
                self.pull_requests.toggle_search(); // Deactivate search mode in widget
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.pull_requests.toggle_search(); // Finalize search / Deactivate search mode in widget
                self.input_mode = InputMode::Normal;
            }
            _ => {
                self.pull_requests.handle_search_input(original_event);
            }
        }
    }

    fn handle_help_input(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('?') => {
                self.pull_requests.toggle_help(); // Deactivate help mode in widget
                self.input_mode = InputMode::Normal;
            }
            _ => {} // Ignore other keys
        }
    }
}
