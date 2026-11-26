use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait Runner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput>;
    fn run_with_stdin(&self, program: &str, args: &[&str], input: &str) -> Result<CommandOutput>;
    fn run_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        _timeout: Duration,
    ) -> Result<CommandOutput> {
        self.run(program, args)
    }

    fn run_streaming(
        &self,
        program: &str,
        args: &[&str],
        _timeout: Duration,
    ) -> Result<CommandOutput> {
        self.run(program, args)
    }
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
        let output = Command::new(program)
            .args(args)
            .output()
            .with_context(|| format!("starting {program}"))?;
        Ok(command_output(output))
    }

    fn run_with_stdin(&self, program: &str, args: &[&str], input: &str) -> Result<CommandOutput> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .with_context(|| format!("writing stdin for {program}"))?;
        }
        let output = child
            .wait_with_output()
            .with_context(|| format!("waiting for {program}"))?;
        Ok(command_output(output))
    }

    fn run_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CommandOutput> {
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        let started = Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                let output = child
                    .wait_with_output()
                    .with_context(|| format!("waiting for {program}"))?;
                return Ok(command_output(output));
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let output = child
                    .wait_with_output()
                    .with_context(|| format!("reaping timed-out {program}"))?;
                let mut result = command_output(output);
                let details = result.stderr.trim();
                result.stderr = if details.is_empty() {
                    format!("timed out after {}s", timeout.as_secs())
                } else {
                    format!("timed out after {}s; {details}", timeout.as_secs())
                };
                result.stdout.clear();
                result.success = false;
                result.code = None;
                return Ok(result);
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn run_streaming(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CommandOutput> {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("starting {program}"))?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(CommandOutput {
                    success: status.success(),
                    code: status.code(),
                    stdout: String::new(),
                    stderr: String::new(),
                });
            }
            if started.elapsed() >= timeout {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                }
                let _ = child.wait();
                return Ok(CommandOutput {
                    success: false,
                    code: None,
                    stdout: String::new(),
                    stderr: format!("timed out after {}s", timeout.as_secs()),
                });
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

fn command_output(output: std::process::Output) -> CommandOutput {
    CommandOutput {
        success: output.status.success(),
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}
