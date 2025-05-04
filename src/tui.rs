mod pr;

use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
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

pub struct App {
    should_quit: bool,
    pull_requests: PullRequestWidget,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 30.0;

    pub fn new(config: Config) -> Self {
        Self {
            should_quit: false,
            pull_requests: PullRequestWidget::new(config),
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
                Some(Ok(event)) = events.next() => self.handle_event(&event).await,
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let [title_area, body_area] = vertical.areas(frame.area());
        let title = Line::from("GitMe PR").centered().bold();
        frame.render_widget(title, title_area);
        frame.render_widget(&self.pull_requests, body_area);
        // Here we need to render the cursor in it's position when we are searching since the api
        // is bind to the frame
        if let Some(cursor_pos) = self.pull_requests.cursor_position() {
            frame.set_cursor_position(cursor_pos);
        }
    }

    async fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    // Handle Esc first to exit modes
                    KeyCode::Esc => {
                        if self.pull_requests.help_open() {
                            self.pull_requests.toggle_help();
                        } else if self.pull_requests.searching() {
                            // Clear and toggle out the search
                            self.pull_requests.clear_search();
                            self.pull_requests.toggle_search();
                        }
                    }
                    KeyCode::Enter => {
                        if self.pull_requests.searching() {
                            self.pull_requests.toggle_search();
                        }
                    }
                    // Handle search input if searching
                    _ if self.pull_requests.searching() => {
                        self.pull_requests.handle_search_input(event);
                    }
                    // Handle help toggle
                    KeyCode::Char('?') => {
                        if !self.pull_requests.help_open() {
                            self.pull_requests.toggle_help();
                        }
                    }
                    // Handle search toggle
                    KeyCode::Char('/') => {
                        if !self.pull_requests.searching() && !self.pull_requests.help_open() {
                            self.pull_requests.toggle_search();
                        }
                    }
                    // Handle other keys only if not searching or in help
                    _ if !self.pull_requests.searching() && !self.pull_requests.help_open() => {
                        match key.code {
                            KeyCode::Char('q') => self.should_quit = true,
                            KeyCode::Char('j') | KeyCode::Down => self.pull_requests.scroll_down(),
                            KeyCode::Char('k') | KeyCode::Up => self.pull_requests.scroll_up(),
                            KeyCode::Char('o') => self.pull_requests.open(),
                            KeyCode::Char('r') => self.pull_requests.review(),
                            KeyCode::Char('f') => self.pull_requests.refresh_pull_requests(),
                            KeyCode::Char('d') => self.pull_requests.jump_down(),
                            KeyCode::Char('u') => self.pull_requests.jump_up(),
                            KeyCode::Tab => self.pull_requests.next_tab(),
                            _ => {}
                        }
                    }
                    _ => {} // Ignore other keys when help is open
                }
            }
        }
    }
}
