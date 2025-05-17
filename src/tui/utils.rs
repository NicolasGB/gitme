use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    widgets::{Block, block::Title},
};

/// Helper function to create a centered rect using percentages.
/// Ensures the rectangle has a minimum size, taking up more relative space
/// if the provided area is small.
pub fn centered_rect(
    area: Rect,
    percent_x: u16,
    percent_y: u16,
    min_width: u16,
    min_height: u16,
) -> Rect {
    // Determine the final dimensions, ensuring they are at least the minimum
    // and do not exceed the available area dimensions.
    let target_width = (area.width as f32 * percent_x as f32 / 100.0) as u16;
    let target_height = (area.height as f32 * percent_y as f32 / 100.0) as u16;

    let final_width = target_width.max(min_width).min(area.width);
    let final_height = target_height.max(min_height).min(area.height);

    let vertical_layout = Layout::vertical([Constraint::Length(final_height)]).flex(Flex::Center);
    let horizontal_layout =
        Layout::horizontal([Constraint::Length(final_width)]).flex(Flex::Center);

    let [centered_vertically] = vertical_layout.areas(area);
    let [final_area] = horizontal_layout.areas(centered_vertically);

    final_area
}

/// Helper function that returns a default block with borders with a given title
pub fn block_with_title<'a>(title: impl Into<Title<'a>>) -> Block<'a> {
    Block::default()
        .title(title)
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
}
