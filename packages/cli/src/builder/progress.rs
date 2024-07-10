//! Report progress about the build to the user. We use channels to report progress back to the CLI.

use anyhow::Context;
use cargo_metadata::{diagnostic::Diagnostic, Message};
use futures_channel::mpsc::UnboundedSender;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::AsyncBufReadExt;
use tracing::Level;

use super::BuildRequest;

#[derive(Default, Debug)]
pub enum Stage {
    #[default]
    Initializing,
    InstallingWasmTooling,
    Compiling,
    OptimizingWasm,
    OptimizingAssets,
    Finished,
}

pub struct UpdateBuildProgress {
    pub stage: Stage,
    pub update: UpdateStage,
}

impl UpdateBuildProgress {
    pub fn to_std_out(&self) {
        match &self.update {
            UpdateStage::Start => match self.stage {
                Stage::Initializing => {
                    println!("--- Initializing ---");
                }
                Stage::InstallingWasmTooling => {
                    println!("--- Installing wasm tooling ---");
                }
                Stage::Compiling => {
                    println!("--- Compiling ---");
                }
                Stage::OptimizingWasm => {
                    println!("--- Optimizing wasm ---");
                }
                Stage::OptimizingAssets => {
                    println!("--- Optimizing assets ---");
                }
                Stage::Finished => {
                    println!("--- Finished ---");
                }
            },
            UpdateStage::AddMessage(message) => match &message.message {
                MessageType::Cargo(message) => {
                    println!("{}", message.rendered.clone().unwrap_or_default());
                }
                MessageType::Text(message) => {
                    println!("{}", message);
                }
            },
            UpdateStage::SetProgress(progress) => {
                println!("Build progress {:0.0}%", progress * 100.0);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStage {
    Start,
    AddMessage(BuildMessage),
    SetProgress(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildMessage {
    pub level: Level,
    pub message: MessageType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    Cargo(Diagnostic),
    Text(String),
}

impl From<Diagnostic> for BuildMessage {
    fn from(message: Diagnostic) -> Self {
        Self {
            level: match message.level {
                cargo_metadata::diagnostic::DiagnosticLevel::Ice
                | cargo_metadata::diagnostic::DiagnosticLevel::FailureNote
                | cargo_metadata::diagnostic::DiagnosticLevel::Error => Level::ERROR,
                cargo_metadata::diagnostic::DiagnosticLevel::Warning => Level::WARN,
                cargo_metadata::diagnostic::DiagnosticLevel::Note => Level::INFO,
                cargo_metadata::diagnostic::DiagnosticLevel::Help => Level::DEBUG,
                _ => Level::DEBUG,
            },
            message: MessageType::Cargo(message),
        }
    }
}

pub(crate) async fn build_cargo(
    crate_count: usize,
    mut cmd: tokio::process::Command,
    progress: &mut UnboundedSender<UpdateBuildProgress>,
) -> anyhow::Result<CargoBuildResult> {
    _ = progress.start_send(UpdateBuildProgress {
        stage: Stage::Compiling,
        update: UpdateStage::Start,
    });

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn cargo build")?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout = tokio::io::BufReader::new(stdout);
    let stderr = tokio::io::BufReader::new(stderr);
    let mut output_location = None;

    let mut stdout = stdout.lines();
    let mut stderr = stderr.lines();
    let mut units_compiled = 0;
    loop {
        let line = tokio::select! {
            line = stdout.next_line() => {
                line
            }
            line = stderr.next_line() => {
                line
            }
        };
        let Some(line) = line? else {
            break;
        };
        let mut deserializer = serde_json::Deserializer::from_str(&line);
        deserializer.disable_recursion_limit();
        let message = Message::deserialize(&mut deserializer).unwrap_or(Message::TextLine(line));
        match message {
            Message::CompilerMessage(msg) => {
                let message = msg.message;
                _ = progress.start_send(UpdateBuildProgress {
                    stage: Stage::Compiling,
                    update: UpdateStage::AddMessage(message.clone().into()),
                });
                const FATAL_LEVELS: &[cargo_metadata::diagnostic::DiagnosticLevel] = &[
                    cargo_metadata::diagnostic::DiagnosticLevel::Error,
                    cargo_metadata::diagnostic::DiagnosticLevel::FailureNote,
                    cargo_metadata::diagnostic::DiagnosticLevel::Ice,
                ];
                if FATAL_LEVELS.contains(&message.level) {
                    return {
                        Err(anyhow::anyhow!(message
                            .rendered
                            .unwrap_or("Unknown".into())))
                    };
                }
            }
            Message::CompilerArtifact(artifact) => {
                units_compiled += 1;
                if let Some(executable) = artifact.executable {
                    output_location = Some(executable.into());
                } else {
                    let build_progress = units_compiled as f64 / crate_count as f64;
                    _ = progress.start_send(UpdateBuildProgress {
                        stage: Stage::Compiling,
                        update: UpdateStage::SetProgress((build_progress).clamp(0.0, 1.00)),
                    });
                }
            }
            Message::BuildScriptExecuted(_) => {
                units_compiled += 1;
            }
            Message::BuildFinished(finished) => {
                if !finished.success {
                    return Err(anyhow::anyhow!("Build failed"));
                }
            }
            Message::TextLine(line) => {
                _ = progress.start_send(UpdateBuildProgress {
                    stage: Stage::Compiling,
                    update: UpdateStage::AddMessage(BuildMessage {
                        level: Level::DEBUG,
                        message: MessageType::Text(line),
                    }),
                });
            }
            _ => {
                // Unknown message
            }
        }
    }

    Ok(CargoBuildResult { output_location })
}

pub(crate) struct CargoBuildResult {
    pub(crate) output_location: Option<PathBuf>,
}

impl BuildRequest {
    /// Try to get the unit graph for the crate. This is a nightly only feature which may not be available with the current version of rustc the user has installed.
    async fn get_unit_count(&self) -> Option<usize> {
        #[derive(Debug, Deserialize)]
        struct UnitGraph {
            units: Vec<serde_json::Value>,
        }

        let mut cmd = tokio::process::Command::new("cargo");
        cmd.arg("+nightly");
        cmd.arg("build");
        cmd.arg("--unit-graph");
        cmd.arg("-Z").arg("unstable-options");

        cmd.args(self.build_arguments());

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let output_text = String::from_utf8(output.stdout).ok()?;
        let graph: UnitGraph = serde_json::from_str(&output_text).ok()?;

        Some(graph.units.len())
    }

    /// Get an estimate of the number of units in the crate. If nightly rustc is not available, this will return an estimate of the number of units in the crate based on cargo metadata.
    /// TODO: always use https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#unit-graph once it is stable
    pub(crate) async fn get_unit_count_estimate(&self) -> usize {
        // Try to get it from nightly
        self.get_unit_count().await.unwrap_or_else(|| {
            // Otherwise, use cargo metadata
            (self
                .dioxus_crate
                .krates
                .krates_filtered(krates::DepKind::Dev)
                .iter()
                .map(|k| k.targets.len())
                .sum::<usize>() as f64
                / 3.5) as usize
        })
    }
}
