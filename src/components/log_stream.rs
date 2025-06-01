use std::collections::VecDeque;

use color_eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Component,
    outer_layout::{self, Message},
};
use crate::{action::Action, config::Config, date::get_diff};
use arboard::Clipboard;

#[derive(Default, Clone, Debug, PartialEq)]
pub struct LogStream {
    /// max: 1000
    received_logs: VecDeque<Message>,

    table_state: TableState,

    saved_logs: Vec<Message>,
}

impl LogStream {
    fn is_follow_log(&self) -> bool {
        // 先頭を選択している場合のみtrue
        self.table_state.selected() == Some(self.received_logs.len())
    }

    fn get_selected_log(&self) -> Option<&Message> {
        if let Some(index) = self.table_state.selected() {
            if let Some(message) = self.received_logs.get(index) {
                return Some(message);
            }
        }
        None
    }
}

impl Component for LogStream {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, action: Action, tx: UnboundedSender<Action>) -> Result<()> {
        if let Action::ComponentAction(action) = action {
            if let Some(action) = action
                .as_any()
                .downcast_ref::<outer_layout::ReceiveNewLog>()
            {
                let is_follow_log = self.is_follow_log();

                self.received_logs.extend(action.new_messages.clone());
                if self.received_logs.len() > 1000 {
                    self.received_logs
                        .drain(0..(self.received_logs.len() - 1000));
                }

                if is_follow_log {
                    self.table_state.select_last();
                }
            }
        }
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: UnboundedSender<Action>,
    ) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Enter => {
                if let Some(selected_index) = self.table_state.selected() {
                    let selected_log = self.received_logs.get(selected_index);
                    if let Some(log) = selected_log {
                        if self.saved_logs.iter().any(|x| x.id == log.id) {
                            self.saved_logs.retain(|x| x.id != log.id);
                        } else {
                            self.saved_logs.push(log.clone());
                        }
                    }
                }
            }

            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::PageUp => {
                self.table_state.scroll_up_by(1);
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::PageDown => {
                self.table_state.scroll_down_by(1);
            }

            crossterm::event::KeyCode::Char('c') => {
                if let Some(message) = self.get_selected_log() {
                    let mut clipboard = Clipboard::new().unwrap();
                    clipboard.set_text(message.url.clone()).unwrap();
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // 縦方向のレイアウトを作成
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .flex(layout::Flex::Center)
            .constraints([Constraint::Fill(1), Constraint::Length(8)])
            .split(area);

        let rows = self
            .received_logs
            .iter()
            .map(|message| {
                let is_highlighted = self.saved_logs.contains(message);
                Row::new(vec![get_diff(message.datetime), message.content.clone()]).style(
                    if is_highlighted {
                        Style::new().bg(Color::Yellow)
                    } else {
                        Style::new()
                    },
                )
            })
            .chain(std::iter::once(
                Row::new(vec!["---", "Follow"]).style(Style::new().fg(Color::Gray)),
            ));
        let table = Table::new(
            rows,
            vec![Constraint::Length(3), Constraint::Percentage(100)],
        )
        .header(
            Row::new(vec!["Tim", "Log"])
                .style(Style::new().bold())
                .bottom_margin(1),
        );

        frame.render_stateful_widget(
            table
                .row_highlight_style(Style::new().reversed())
                .highlight_symbol(">")
                .block(Block::bordered().title("Log Stream")),
            chunks[0],
            &mut self.table_state,
        );

        if let Some(message) = self.get_selected_log() {
            frame.render_widget(
                Paragraph::new(message.content.clone())
                    .block(Block::bordered().title(format!("Log Detail | {}", message.datetime))),
                chunks[1],
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::collections::VecDeque;

    #[test]
    fn test_dequeue() {
        let mut a: VecDeque<i32> = VecDeque::new();
        a.extend([1, 2, 3]);
        a.extend([4, 5, 6]);
        a.extend([7, 8, 9]);

        assert_eq!(a.len(), 9);

        a.drain(0..3);

        assert_eq!(a.len(), 6);
        assert_eq!(a[0], 4);
        assert_eq!(a[1], 5);
        assert_eq!(a[2], 6);
        a.extend([10, 11]);
        a.drain(0..2);

        assert_eq!(a.len(), 6);
        assert_eq!(a[0], 6);
        assert_eq!(a[1], 7);
    }
}
