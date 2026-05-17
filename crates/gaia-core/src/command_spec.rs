use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub working_dir: Option<PathBuf>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);

        if let Some(dir) = &self.working_dir {
            command.current_dir(dir);
        }

        for (key, value) in &self.env {
            command.env(key, value);
        }

        command
    }

    pub fn to_shell_string(&self) -> String {
        let mut pieces = vec![shell_quote(&self.program)];
        pieces.extend(self.args.iter().map(|arg| shell_quote(arg)));
        pieces.join(" ")
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_owned();
    }

    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./=:".contains(ch))
    {
        return value.to_owned();
    }

    format!("'{}'", value.replace('\'', r"'\''"))
}
