use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,
    ComponentAction(Box<dyn ComponentAction>),
}

impl Clone for Action {
    fn clone(&self) -> Self {
        match self {
            Action::Tick => Action::Tick,
            Action::Render => Action::Render,
            Action::Resize(w, h) => Action::Resize(*w, *h),
            Action::Suspend => Action::Suspend,
            Action::Resume => Action::Resume,
            Action::Quit => Action::Quit,
            Action::ClearScreen => Action::ClearScreen,
            Action::Error(msg) => Action::Error(msg.clone()),
            Action::Help => Action::Help,
            Action::ComponentAction(action) => Action::ComponentAction(action.clone_box()),
        }
    }
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Action::Tick, Action::Tick) => true,
            (Action::Render, Action::Render) => true,
            (Action::Resize(w1, h1), Action::Resize(w2, h2)) => w1 == w2 && h1 == h2,
            (Action::Suspend, Action::Suspend) => true,
            (Action::Resume, Action::Resume) => true,
            (Action::Quit, Action::Quit) => true,
            (Action::ClearScreen, Action::ClearScreen) => true,
            (Action::Error(msg1), Action::Error(msg2)) => msg1 == msg2,
            (Action::Help, Action::Help) => true,
            (Action::ComponentAction(a1), Action::ComponentAction(a2)) => a1.name() == a2.name(),
            _ => false,
        }
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let action = String::deserialize(deserializer)?;
        match action.as_str() {
            "Tick" => Ok(Action::Tick),
            "Render" => Ok(Action::Render),
            "Resize" => Ok(Action::Resize(0, 0)),
            "Suspend" => Ok(Action::Suspend),
            "Resume" => Ok(Action::Resume),
            "Quit" => Ok(Action::Quit),
            "ClearScreen" => Ok(Action::ClearScreen),
            "Error" => Ok(Action::Error(String::new())),
            "Help" => Ok(Action::Help),
            _ => Err(serde::de::Error::unknown_variant(
                &action,
                &[
                    "Tick",
                    "Render",
                    "Resize",
                    "Suspend",
                    "Resume",
                    "Quit",
                    "ClearScreen",
                    "Error",
                    "Help",
                ],
            )),
        }
    }
}

pub trait ComponentAction: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;
    fn clone_box(&self) -> Box<dyn ComponentAction>;
    fn as_any(&self) -> &dyn std::any::Any;
}
