use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use color_eyre::Result;
use ratatui::{prelude::*, widgets::*};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use super::{
    Component,
    outer_layout::{self, Message},
};
use crate::action::ComponentAction;
use crate::notification::show_notification;
use crate::{action::Action, config::Config, date::get_diff};
use arboard::Clipboard;

#[derive(Clone, Debug, PartialEq)]
struct ExportLogs {
    filepath: String,
}

impl ComponentAction for ExportLogs {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "ExportLogs"
    }

    fn clone_box(&self) -> Box<dyn ComponentAction> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectLog {
    pub selected_log: Message,
}

impl ComponentAction for SelectLog {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "SelectLog"
    }

    fn clone_box(&self) -> Box<dyn ComponentAction> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
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

    fn export_saved_logs(&mut self) -> Result<String> {
        if self.saved_logs.is_empty() {
            return Ok(String::new());
        }

        let now = Utc::now();
        let filename = format!(
            "saved_logs_{}.jsonl",
            now.with_timezone(&Tokyo).format("%Y%m%d_%H%M%S")
        );
        let mut file = File::create(&filename)?;
        for message in &self.saved_logs {
            writeln!(file, "{}", message.content)?;
        }

        Ok(filename)
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
                if let Some(selected_log) = self.get_selected_log() {
                    tx.send(Action::ComponentAction(Box::new(SelectLog {
                        selected_log: selected_log.clone(),
                    })))?;
                }
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::PageDown => {
                self.table_state.scroll_down_by(1);
                if let Some(selected_log) = self.get_selected_log() {
                    tx.send(Action::ComponentAction(Box::new(SelectLog {
                        selected_log: selected_log.clone(),
                    })))?;
                }
            }

            crossterm::event::KeyCode::Char('c') => {
                if let Some(message) = self.get_selected_log() {
                    let mut clipboard = Clipboard::new().unwrap();
                    clipboard.set_text(message.url.clone()).unwrap();

                    show_notification(
                        "Copy URL",
                        &format!("Copied URL to clipboard: {}", message.url),
                    );
                }
            }
            crossterm::event::KeyCode::Char('e') => {
                if let Ok(path) = self.export_saved_logs() {
                    show_notification("Log Export", &format!("Exported logs to {}", path));
                } else {
                    show_notification("Log Export", "Failed to export logs.");
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let rows = self
            .received_logs
            .iter()
            .map(|message| {
                let is_highlighted = self.saved_logs.contains(message);
                let content_line = convert_to_line(&message.content);
                Row::new(vec![Line::from(get_diff(message.datetime)), content_line]).style(
                    if is_highlighted {
                        Style::new().bg(Color::Yellow)
                    } else {
                        Style::new()
                    },
                )
            })
            .chain(std::iter::once(
                Row::new(vec![Line::from("---"), Line::from("Follow")])
                    .style(Style::new().fg(Color::Gray)),
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
            area,
            &mut self.table_state,
        );
        Ok(())
    }
}

pub fn convert_to_line(raw_text: &str) -> Line<'static> {
    let result: Result<Value, _> = serde_json::from_str(raw_text);

    if result.is_err() {
        return Line::from(raw_text.to_string());
    }

    let value = result.unwrap();
    if !value.is_object() {
        return Line::from(raw_text.to_string());
    }

    let obj = value.as_object().unwrap();
    if !obj.contains_key("message") {
        return Line::from(raw_text.to_string());
    }

    let mut spans = Vec::new();
    spans.push(Span::raw("{"));

    for (key, value) in obj.iter() {
        spans.push(Span::raw(format!("\"{}\":", key)));
        if key == "message" {
            // messageプロパティは階層的に色付け
            format_value_with_colors(&mut spans, value, 0);
        } else {
            // その他のプロパティは通常の色で表示
            spans.push(Span::raw(
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
            ));
        }
        spans.push(Span::raw(","));
    }

    // 最後のカンマを削除
    if let Some(last_span) = spans.last_mut() {
        if last_span.content.ends_with(",") {
            *last_span = Span::raw(last_span.content.trim_end_matches(",").to_string());
        }
    }

    spans.push(Span::raw("}"));
    Line::from(spans)
}

fn format_value_with_colors(spans: &mut Vec<Span<'static>>, value: &Value, depth: usize) {
    let colors = [
        Color::LightRed,
        Color::LightBlue,
        Color::LightCyan,
        Color::LightMagenta,
        Color::LightGreen,
    ];
    let color = colors[depth % colors.len()];

    match value {
        Value::Object(obj) => {
            spans.push(Span::styled("{", Style::default().fg(color)));
            for (i, (key, val)) in obj.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(",", Style::default().fg(color)));
                }
                spans.push(Span::styled(
                    format!("\"{}\":", key),
                    Style::default().fg(color),
                ));
                format_value_with_colors(spans, val, depth + 1);
            }
            spans.push(Span::styled("}", Style::default().fg(color)));
        }
        Value::Array(arr) => {
            spans.push(Span::styled("[", Style::default().fg(color)));
            for (i, val) in arr.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(",", Style::default().fg(color)));
                }
                format_value_with_colors(spans, val, depth + 1);
            }
            spans.push(Span::styled("]", Style::default().fg(color)));
        }
        _ => {
            spans.push(Span::styled(
                serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
                Style::default().fg(color),
            ));
        }
    }
}

#[cfg(test)]
mod test {

    use std::collections::VecDeque;

    use super::*;

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

    #[test]
    fn test_convert_to_line() {
        let a = convert_to_line(
            r#"
{
    "cold_start": true,
    "function_arn": "arn:aws:lambda:ap-northeast-1:XXXXXXXXXXXX:function:AwsAppStack-loggerE71C1604-7O0zP2rA8iBZ",
    "function_memory_size": 128,
    "function_name": "AwsAppStack-loggerE71C1604-7O0zP2rA8iBZ",
    "function_request_id": "5c4e6be5-870f-487a-a0e8-bc05d44f1d9c",
    "level": "INFO",
    "message": "This is an INFO log with some context",
    "service": "shopping-cart-api",
    "timestamp": "2022-07-23T09:33:23.238Z",
    "xray_trace_id": "1-62dbc062-2b65813062b227f4358eb9c1",
    "event": {
        "key1": "value1",
        "key2": "value2",
        "key3": "value3"
    }
}
        "#,
        );
    }
}
