use color_eyre::Result;
use ratatui::{
    prelude::*,
    widgets::{Block, Paragraph, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    action::Action,
    components::{
        log_stream::{SelectLog, convert_to_line},
        outer_layout::Message,
    },
};

#[derive(Clone, Debug)]
struct MessageAndLine<'a> {
    message: Message,
    content_line: Line<'a>,
}

#[derive(Default, Clone, Debug)]
pub struct LogDetail<'a> {
    message_and_line: Option<MessageAndLine<'a>>,
}

impl<'a> LogDetail<'a> {
    pub fn new() -> Self {
        Self {
            message_and_line: None,
        }
    }

    pub fn update(&mut self, action: Action, _tx: UnboundedSender<Action>) -> Result<()> {
        match action {
            Action::ComponentAction(component_action) => {
                if let Some(select_log_action) =
                    component_action.as_any().downcast_ref::<SelectLog>()
                {
                    self.message_and_line = Some(MessageAndLine {
                        content_line: convert_to_line(&select_log_action.selected_log.content),
                        message: select_log_action.selected_log.clone(),
                    });
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        if let Some(message_and_line) = &self.message_and_line {
            // Create vertical layout: datetime, url, content
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // datetime
                    Constraint::Length(1), // url
                    Constraint::Fill(1),   // content
                ])
                .split(area);

            // Draw datetime
            frame.render_widget(
                Paragraph::new(format!("DateTime: {}", message_and_line.message.datetime))
                    .style(Style::default().fg(Color::Cyan)),
                chunks[0],
            );

            // Draw url
            frame.render_widget(
                Paragraph::new(format!("URL: {}", message_and_line.message.url))
                    .style(Style::default().fg(Color::Green)),
                chunks[1],
            );

            // Draw content
            frame.render_widget(
                Paragraph::new(message_and_line.content_line.clone())
                    .wrap(Wrap { trim: true })
                    .block(Block::bordered().title("Log Content")),
                chunks[2],
            );
        }

        Ok(())
    }
}
