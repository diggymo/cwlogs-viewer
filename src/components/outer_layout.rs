use chrono::DateTime;
use chrono_tz::Tz;
use color_eyre::Result;
use ratatui::prelude::*;
use serde::{Serialize, Serializer};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use ulid::Ulid;

use super::{
    Component,
    log_group_list::{self, LogGroupList},
    log_stream::LogStream,
};
use crate::{
    action::{Action, ComponentAction},
    config::Config,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Message {
    pub id: Ulid,
    pub content: String,
    pub datetime: DateTime<Tz>,
    pub url: String,
}
impl Serialize for Message {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.content)
    }
}

impl Message {
    fn generate_url(log_group_identifier: &str, log_stream_name: &str) -> String {
        let log_group_id_without_account = log_group_identifier
            .replace(ACCOUNT_ID, "")
            .replace(":", "");

        format!(
            "https://{}.console.aws.amazon.com/cloudwatch/home?region={}#logsV2:log-groups/log-group/{}/log-events/{}",
            AWS_REGION,
            AWS_REGION,
            urlencoding::encode(&urlencoding::encode(&log_group_id_without_account)),
            urlencoding::encode(&urlencoding::encode(log_stream_name))
        )
    }
}

const AWS_REGION: &str = "ap-northeast-1";
const ACCOUNT_ID: &str = "153820248175";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReceiveNewLog {
    pub new_messages: Vec<Message>,
}
impl ComponentAction for ReceiveNewLog {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &'static str {
        "ReceiveNewLog"
    }

    fn clone_box(&self) -> Box<dyn ComponentAction> {
        Box::new(self.clone())
    }
}

#[derive(Clone, Debug)]
enum Cursor {
    LogGroupList,
    LogStream,
}
impl Default for Cursor {
    fn default() -> Self {
        Self::LogGroupList
    }
}

#[derive(Default, Clone, Debug)]
pub struct OuterLayout {
    cursor: Cursor,
    log_group_list: LogGroupList,
    log_stream: LogStream,
    stream_cancel_token: Option<CancellationToken>,
}

impl OuterLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_live_tail(
        &mut self,
        log_group_arn_list: Vec<String>,
        tx: UnboundedSender<Action>,
    ) {
        // 既存のlive tailがあれば停止
        self.stop_live_tail();

        if log_group_arn_list.is_empty() {
            return;
        }

        // 新しいキャンセレーショントークンを作成
        let cancel_token = CancellationToken::new();
        self.stream_cancel_token = Some(cancel_token.clone());

        tokio::spawn(async move {
            // Initialize AWS SDK
            let config = aws_config::load_from_env().await;
            let client = aws_sdk_cloudwatchlogs::Client::new(&config);
            let mut stream = client
                .start_live_tail()
                .set_log_group_identifiers(Some(log_group_arn_list))
                .send()
                .await
                .unwrap()
                .response_stream;

            loop {
                tokio::select! {
                    // キャンセルシグナルを監視
                    _ = cancel_token.cancelled() => {
                        debug!("Live tail cancelled");
                        break;
                    }
                    // ストリームからのデータを処理
                    result = stream.recv() => {
                        match result {
                            Ok(Some(log_event)) => {
                                if log_event.is_session_start() {
                                    continue;
                                }

                                let new_messages = log_event
                                    .as_session_update()
                                    .unwrap()
                                    .session_results
                                    .as_ref()
                                    .unwrap()
                                    .iter()
                                    .map(|session_result| {
                                        Message {
                                            id: Ulid::new(),
                                            content: session_result.message.as_ref().unwrap().to_string(),
                                            datetime: DateTime::from_timestamp_millis(
                                                session_result.timestamp.unwrap(),
                                            )
                                            .unwrap()
                                            .with_timezone(&chrono_tz::Asia::Tokyo),
                                            url: Message::generate_url(
                                                session_result.log_group_identifier.as_ref().unwrap(),
                                                session_result.log_stream_name.as_ref().unwrap(),
                                            ),
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                if new_messages.is_empty() {
                                    // let id = Ulid::new();
                                    // tx.send(Action::ComponentAction(Box::new(ReceiveNewLog {
                                    //     new_messages: vec![Message {
                                    //         id,
                                    //         url: format!("https://ap-northeast-1.console.aws.amazon.com/cloudwatch/home?region=ap-northeast-1#logsV2:log-groups/log-group/{}"),
                                    //         content: format!("hoge{}", id),
                                    //         datetime: Local::now().with_timezone(&Tokyo),
                                    //     }],
                                    // }))).unwrap();
                                    debug!("No new messages in this log event.");
                                    continue;
                                }

                                debug!("Received new_messages: {:?}", &new_messages);
                                if tx.send(Action::ComponentAction(Box::new(ReceiveNewLog {
                                    new_messages,
                                }))).is_err() {
                                    debug!("Failed to send new messages - receiver dropped");
                                    break;
                                }
                            }
                            Ok(None) => {
                                debug!("No more log events to process.");
                                break;
                            }
                            Err(e) => {
                                debug!("Error receiving log events: {:?}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn stop_live_tail(&mut self) {
        if let Some(cancel_token) = &self.stream_cancel_token {
            cancel_token.cancel();
            self.stream_cancel_token = None;
            debug!("Live tail stopped");
        }
    }
}

impl Component for OuterLayout {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.log_group_list.register_action_handler(tx.clone())?;
        self.log_stream.register_action_handler(tx)?;
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        Ok(())
    }

    fn update(&mut self, action: Action, tx: UnboundedSender<Action>) -> Result<()> {
        self.log_group_list.update(action.clone(), tx.clone())?;
        self.log_stream.update(action.clone(), tx.clone())?;

        if let Action::ComponentAction(action) = action {
            if let Some(action) = action
                .as_any()
                .downcast_ref::<log_group_list::SelectLogGroup>()
            {
                debug!("Log group list updated with {:?} items", &action);
                self.start_live_tail(
                    action
                        .log_groups
                        .clone()
                        .into_iter()
                        .map(|lg| lg.arn)
                        .collect(),
                    tx,
                );
            }
        }
        Ok(())
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        tx: UnboundedSender<Action>,
    ) -> Result<()> {
        match self.cursor {
            Cursor::LogGroupList => {
                if key.code == crossterm::event::KeyCode::Tab {
                    self.cursor = Cursor::LogStream;
                    return Ok(());
                }
                self.log_group_list.handle_key_event(key, tx.clone())?;
            }
            Cursor::LogStream => {
                if key.code == crossterm::event::KeyCode::Tab {
                    self.cursor = Cursor::LogGroupList;
                    return Ok(());
                }

                self.log_stream.handle_key_event(key, tx)?;
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let constraints = match self.cursor {
            Cursor::LogGroupList => vec![Constraint::Percentage(70), Constraint::Percentage(30)],
            Cursor::LogStream => vec![Constraint::Percentage(30), Constraint::Percentage(70)],
        };

        let outer_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        // Log group listの高さを20に制限
        // let left_layout = Layout::default()
        //     .direction(Direction::Vertical)
        //     .constraints(vec![Constraint::Length(20), Constraint::Min(0)])
        //     .split(outer_layout[0]);

        self.log_group_list.draw(frame, outer_layout[0])?;
        self.log_stream.draw(frame, outer_layout[1])?;
        Ok(())
    }
}

/*

https://ap-northeast-1.console.aws.amazon.com/cloudwatch/home?region=ap-northeast-1#logsV2:log-groups/log-group/$252Faws$252Flambda$252FAppStack-CustomCDKBucketDeployment8693BB64968944B6-GQdSBL5N4uaZ/log-events/2025$252F04$252F21$252F$255B$2524LATEST$255Dd9c069780d294516942fcf15418b401f
https://ap-northeast-1.console.aws.amazon.com/cloudwatch/home?region=ap-northeast-1#logsV2:log-groups/log-group/153820248175%253A%252Faws%252Flambda%252FDevAppStackReservationStack8EF3E1A5-update46C5CAF0-yy7WH9VPDYs8/log-events/2025%252F03%252F12%252F%255B%2524LATEST%255Da30225cd3ef94b7a8d76d01fab306abe

*/
