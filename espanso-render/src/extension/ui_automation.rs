/*
 * This file is part of espanso.
 *
 * Copyright (C) 2019-2025 Vicentini Diego
 *
 * espanso is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * espanso is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with espanso.  If not, see <https://www.gnu.org/licenses/>.
 */

use crate::{Extension, ExtensionOutput, ExtensionResult, Params, Value};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;
use thiserror::Error;

pub struct UiAutomationExtension {
    alias: String,
}

impl UiAutomationExtension {
    pub fn new() -> Self {
        Self {
            alias: "ui-automation".to_string(),
        }
    }
}

impl Default for UiAutomationExtension {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
enum Command {
    Sleep(f64),
    Type(String),
    Press(Key),
    Release(Key),
    Tap(Key),
}

impl Extension for UiAutomationExtension {
    fn name(&self) -> &str {
        self.alias.as_str()
    }

    fn calculate(
        &self,
        _: &crate::Context,
        scope: &crate::Scope,
        params: &Params,
    ) -> ExtensionResult {
        let script = params.get("script");
        if script.is_none() {
            return ExtensionResult::Error(
                UiAutomationError::MissingScriptParameter.into()
            );
        }

        let script_str = match script.unwrap() {
            Value::String(s) => s,
            _ => {
                return ExtensionResult::Error(
                    UiAutomationError::InvalidScriptParameter.into()
                );
            }
        };

        let (trigger_value, trigger_length) = if let Some(ExtensionOutput::Single(trigger_value)) = scope.get("_trigger_") {
            let len = trigger_value.len();
            (trigger_value.clone(), len)
        } else {
            (String::new(), 0)
        };

        let mut commands = match parse_script(script_str) {
            Ok(cmds) => cmds,
            Err(e) => return ExtensionResult::Error(e.into()),
        };

        // Insert backspace commands at the beginning to delete the trigger
        for _ in 0..trigger_length {
            commands.insert(0, Command::Tap(Key::Backspace));
        }

        // Add type command at the end to restore the trigger
        if !trigger_value.is_empty() {
            commands.push(Command::Type(trigger_value));
        }

        // Execute commands
        if let Err(e) = execute_commands(&commands) {
            return ExtensionResult::Error(e.into());
        }

        ExtensionResult::Success(ExtensionOutput::Single(String::new()))
    }
}

fn parse_script(script: &str) -> Result<Vec<Command>, UiAutomationError> {
    let mut commands = Vec::new();

    for line in script.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let cmd = parts[0];

        match cmd {
            "sleep" => {
                if parts.len() < 2 {
                    return Err(UiAutomationError::InvalidCommand(line.to_string()));
                }
                let duration: f64 = parts[1]
                    .parse()
                    .map_err(|_| UiAutomationError::InvalidCommand(line.to_string()))?;
                commands.push(Command::Sleep(duration));
            }
            "type" => {
                if parts.len() < 2 {
                    return Err(UiAutomationError::InvalidCommand(line.to_string()));
                }
                commands.push(Command::Type(parts[1].to_string()));
            }
            "press" => {
                if parts.len() < 2 {
                    return Err(UiAutomationError::InvalidCommand(line.to_string()));
                }
                let key = parse_key(parts[1])?;
                commands.push(Command::Press(key));
            }
            "release" => {
                if parts.len() < 2 {
                    return Err(UiAutomationError::InvalidCommand(line.to_string()));
                }
                let key = parse_key(parts[1])?;
                commands.push(Command::Release(key));
            }
            "tap" => {
                if parts.len() < 2 {
                    return Err(UiAutomationError::InvalidCommand(line.to_string()));
                }
                let key = parse_key(parts[1])?;
                commands.push(Command::Tap(key));
            }
            _ => {
                return Err(UiAutomationError::UnknownCommand(cmd.to_string()));
            }
        }
    }

    Ok(commands)
}

fn parse_key(key_name: &str) -> Result<Key, UiAutomationError> {
    match key_name.to_lowercase().as_str() {
        "tab" => Ok(Key::Tab),
        "return" | "enter" => Ok(Key::Return),
        "shift" => Ok(Key::Shift),
        "control" | "ctrl" => Ok(Key::Control),
        "alt" => Ok(Key::Alt),
        "delete" => Ok(Key::Delete),
        "cmd" | "meta" | "super" => Ok(Key::Meta),
        "backspace" | "back" => Ok(Key::Backspace),
        "capslock" => Ok(Key::CapsLock),
        "page-up" => Ok(Key::PageUp),
        "page-down" => Ok(Key::PageDown),
        _ => Err(UiAutomationError::UnknownKey(key_name.to_string())),
    }
}

fn execute_commands(commands: &[Command]) -> Result<(), UiAutomationError> {
    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings)
        .map_err(|e| UiAutomationError::EnigoInitError(format!("{:?}", e)))?;

    for command in commands {
        match command {
            Command::Sleep(seconds) => {
                let millis = (seconds * 1000.0) as u64;
                thread::sleep(Duration::from_millis(millis));
            }
            Command::Type(text) => {
                for ch in text.chars() {
                    enigo.key(Key::Unicode(ch), enigo::Direction::Click)
                        .map_err(|e| UiAutomationError::EnigoError(format!("{:?}", e)))?;
                }
            }
            Command::Press(key) => {
                enigo.key(*key, enigo::Direction::Press)
                    .map_err(|e| UiAutomationError::EnigoError(format!("{:?}", e)))?;
            }
            Command::Release(key) => {
                enigo.key(*key, enigo::Direction::Release)
                    .map_err(|e| UiAutomationError::EnigoError(format!("{:?}", e)))?;
            }
            Command::Tap(key) => {
                enigo.key(*key, enigo::Direction::Click)
                    .map_err(|e| UiAutomationError::EnigoError(format!("{:?}", e)))?;
            }
        }
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum UiAutomationError {
    #[error("missing 'script' parameter")]
    MissingScriptParameter,

    #[error("invalid 'script' parameter, expected string")]
    InvalidScriptParameter,

    #[error("invalid command: {0}")]
    InvalidCommand(String),

    #[error("unknown command: {0}")]
    UnknownCommand(String),

    #[error("unknown key: {0}")]
    UnknownKey(String),

    #[error("failed to initialize enigo: {0}")]
    EnigoInitError(String),

    #[error("enigo error: {0}")]
    EnigoError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_sleep() {
        let script = "sleep 1.5";
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            Command::Sleep(duration) => assert_eq!(*duration, 1.5),
            _ => panic!("Expected Sleep command"),
        }
    }

    #[test]
    fn test_parse_type() {
        let script = "type Hello World";
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            Command::Type(text) => assert_eq!(text, "Hello World"),
            _ => panic!("Expected Type command"),
        }
    }

    #[test]
    fn test_parse_press() {
        let script = "press shift";
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 1);
        matches!(&commands[0], Command::Press(Key::Shift));
    }

    #[test]
    fn test_parse_release() {
        let script = "release shift";
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 1);
        matches!(&commands[0], Command::Release(Key::Shift));
    }

    #[test]
    fn test_parse_tap() {
        let script = "tap enter";
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 1);
        matches!(&commands[0], Command::Tap(Key::Return));
    }

    #[test]
    fn test_parse_complex_script() {
        let script = r#"
sleep 1
type Liebe Grüße
press shift
type uppercase
release shift
tap enter
type Robert
tap tab
type finish
"#;
        let commands = parse_script(script).unwrap();
        assert_eq!(commands.len(), 9);
    }

    #[test]
    fn test_missing_script_parameter() {
        let extension = UiAutomationExtension::new();
        let params = Params::new();
        let result = extension.calculate(&crate::Context::default(), &HashMap::default(), &params);
        assert!(matches!(result, ExtensionResult::Error(_)));
    }

    #[test]
    fn test_invalid_command() {
        let script = "invalid command";
        let result = parse_script(script);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_key() {
        let script = "tap unknownkey";
        let result = parse_script(script);
        assert!(result.is_err());
    }
}
