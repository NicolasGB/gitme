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

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| self.draw(frame))?; },
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
    }

    async fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                    KeyCode::Char('j') | KeyCode::Down => self.pull_requests.scroll_down(),
                    KeyCode::Char('k') | KeyCode::Up => self.pull_requests.scroll_up(),
                    KeyCode::Char('o') => self.pull_requests.open(),
                    KeyCode::Char('r') => self.pull_requests.review(),
                    KeyCode::Char('z') => self.pull_requests.expand_all(),
                    KeyCode::Char('c') => self.pull_requests.contract_all(),
                    KeyCode::Enter => self.pull_requests.toggle_expand(),
                    KeyCode::Tab => self.pull_requests.next_tab(),
                    _ => {}
                }
            }
        }
    }
}
