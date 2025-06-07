use std::collections::HashSet;
use std::env;
use std::fs;

use chrono::{DateTime, Utc};
use chrono_tz::{Asia::Tokyo, Tz};
use color_eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use super::Component;
use crate::{
    action::{Action, ComponentAction},
    config::Config,
    date::get_diff,
};

#[derive(Clone, Debug, PartialEq)]
pub struct LogGroup {
    pub name: String,
    pub arn: String,
    creation_time: DateTime<Tz>,
}

impl Default for LogGroup {
    fn default() -> Self {
        Self {
            name: String::new(),
            arn: String::new(),
            creation_time: Utc::now().with_timezone(&Tokyo),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FetchLogGroups {
    pub log_groups: Vec<LogGroup>,
}
impl ComponentAction for FetchLogGroups {
    fn name(&self) -> &'static str {
        "FetchLogGroups"
    }

    fn clone_box(&self) -> Box<dyn ComponentAction> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectLogGroup {
    pub log_groups: Vec<LogGroup>,
}
impl ComponentAction for SelectLogGroup {
    fn name(&self) -> &'static str {
        "SelectLogGroup"
    }

    fn clone_box(&self) -> Box<dyn ComponentAction> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogGroupList {
    loaded_log_groups: Vec<LogGroup>,
    table_state: TableState,

    selected_log_groups: HashSet<usize>,
}

impl Default for LogGroupList {
    fn default() -> Self {
        let mut logs = Vec::new();

        (0..100).for_each(|i| {
            logs.push(LogGroup {
                creation_time: Utc::now().with_timezone(&Tokyo)
                    - std::time::Duration::from_secs(i as u64 * 60), // Simulate creation time
                name: format!("LogGroup{}", i),
                arn: format!(
                    "arn:aws:logs:us-west-2:123456789012:log-group:LogGroup{}",
                    i
                ),
            });
        });

        Self {
            loaded_log_groups: logs,
            selected_log_groups: HashSet::new(),
            table_state: TableState::default(),
        }
    }
}

impl LogGroupList {
    fn save_selected_log_groups(&self) {
        let temp_dir = env::temp_dir();
        let filepath = temp_dir.join("cwlogs-viewer-selected-groups.txt");

        let selected_names: Vec<String> = self
            .selected_log_groups
            .iter()
            .filter_map(|&index| self.loaded_log_groups.get(index))
            .map(|lg| lg.name.clone())
            .collect();

        let content = selected_names.join("\n");

        if let Err(e) = fs::write(&filepath, content) {
            debug!("Failed to save selected log groups: {}", e);
        } else {
            debug!(
                "Saved {} selected log groups to {:?}",
                selected_names.len(),
                filepath
            );
        }
    }

    fn load_selected_log_groups(&mut self) {
        let temp_dir = env::temp_dir();
        let filepath = temp_dir.join("cwlogs-viewer-selected-groups.txt");

        if let Ok(content) = fs::read_to_string(&filepath) {
            let saved_names: HashSet<String> = content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.to_string())
                .collect();

            self.selected_log_groups.clear();
            for (index, log_group) in self.loaded_log_groups.iter().enumerate() {
                if saved_names.contains(&log_group.name) {
                    self.selected_log_groups.insert(index);
                }
            }

            debug!(
                "Loaded {} selected log groups from {:?}",
                self.selected_log_groups.len(),
                filepath
            );
        } else {
            debug!("No saved selected log groups found at {:?}", filepath);
        }
    }
}

impl Component for LogGroupList {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        tokio::spawn(async move {
            // Initialize AWS SDK
            debug!("Initializing AWS SDK for CloudWatch Logs");
            let config = aws_config::load_from_env().await;
            let client = aws_sdk_cloudwatchlogs::Client::new(&config);

            let mut log_groups: Vec<LogGroup> = client
                .describe_log_groups()
                .into_paginator()
                .send()
                .try_collect()
                .await
                .map_err(|e| {
                    debug!("Failed to list log groups: {}", e);
                    e
                })
                .into_iter()
                .flat_map(|res| {
                    res.into_iter()
                        .flat_map(|group| group.log_groups.unwrap())
                })
                .map(|log_group| LogGroup {
                    creation_time: DateTime::from_timestamp_millis(
                        log_group.creation_time.unwrap(),
                    )
                    .unwrap()
                    .with_timezone(&chrono_tz::Asia::Tokyo),
                    name: log_group.log_group_name.unwrap_or_default(),
                    arn: log_group.log_group_arn.unwrap_or_default(),
                })
                .collect();

            log_groups.sort_by(|a, b| {
                b.creation_time
                    .partial_cmp(&a.creation_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            tx.send(Action::ComponentAction(Box::new(FetchLogGroups {
                log_groups,
            })))
            .unwrap_or_else(|e| {
                debug!("Failed to send FetchLogGroups action: {}", e);
            })
        });

        Ok(())
    }

    fn register_config_handler(&mut self, _config: Config) -> Result<()> {
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: UnboundedSender<Action>,
    ) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::PageUp => {
                self.table_state.scroll_up_by(1);
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::PageDown => {
                self.table_state.scroll_down_by(1);
            }

            crossterm::event::KeyCode::Enter => {
                if let Some(selected_index) = self.table_state.selected() {
                    if self.selected_log_groups.contains(&selected_index) {
                        self.selected_log_groups.remove(&selected_index);
                    } else if self.selected_log_groups.len() < 10 {
                        self.selected_log_groups.insert(selected_index);
                    } else {
                        debug!("Cannot select more than 10 log groups");
                        return Ok(());
                    }

                    // get selected log groups from self.log_groups
                    let selected_log_groups: Vec<LogGroup> = self
                        .selected_log_groups
                        .iter()
                        .filter_map(|&index| self.loaded_log_groups.get(index).cloned())
                        .collect();

                    self.save_selected_log_groups();

                    tx.send(Action::ComponentAction(Box::new(SelectLogGroup {
                        log_groups: selected_log_groups,
                    })))?;
                }
            }
            _ => {}
        };

        Ok(())
    }

    fn update(&mut self, action: Action, tx: UnboundedSender<Action>) -> Result<()> {
        match action {
            Action::ComponentAction(component_action) => {
                if let Some(fetch_action) =
                    component_action.as_any().downcast_ref::<FetchLogGroups>()
                {
                    self.loaded_log_groups = fetch_action.log_groups.clone();
                    debug!(
                        "Updated log groups with {} items",
                        self.loaded_log_groups.len()
                    );

                    self.load_selected_log_groups();

                    if !self.selected_log_groups.is_empty() {
                        let selected_log_groups: Vec<LogGroup> = self
                            .selected_log_groups
                            .iter()
                            .filter_map(|&index| self.loaded_log_groups.get(index).cloned())
                            .collect();

                        if !selected_log_groups.is_empty() {
                            tx.send(Action::ComponentAction(Box::new(SelectLogGroup {
                                log_groups: selected_log_groups,
                            })))?;
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let rows = self
            .loaded_log_groups
            .iter()
            .enumerate()
            .map(|(i, log_group)| {
                let text = log_group.name.to_string();
                let is_highlighted = self.selected_log_groups.contains(&i);
                Row::new(vec![get_diff(log_group.creation_time), text]).style(if is_highlighted {
                    Style::new().bg(Color::Yellow)
                } else {
                    Style::new()
                })
            });
        let table = Table::new(
            rows,
            vec![Constraint::Length(3), Constraint::Percentage(100)],
        )
        .header(
            Row::new(vec!["Cre", "LogGroup"])
                .style(Style::new().bold())
                .bottom_margin(1),
        );

        frame.render_stateful_widget(
            table
                .row_highlight_style(Style::new().reversed())
                .highlight_symbol(">")
                .block(Block::bordered().title("Log Group List")),
            area,
            &mut self.table_state,
        );
        Ok(())
    }
}

#[cfg(test)]
mod test {
    

    // #[test]
    // fn test_creation_time() {
    //     // 1433189500783ミリ秒 = 2015-06-02T05:11:40.783Z
    //     let timestamp_ms = 1433189500783;
    //     let system_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(timestamp_ms);

    //     // JST形式（日本標準時）で出力
    //     let utc_time: DateTime<Utc> = system_time.into();
    //     let jst_time = utc_time.with_timezone(&Tokyo);
    //     dbg!(jst_time.to_rfc3339());
    // }
}
