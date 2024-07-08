use crate::build::{self, Build};
use crate::builder::BuildRequest;
use crate::builder::BuildResult;
use crate::dioxus_crate::DioxusCrate;
use crate::serve::Serve;
use crate::Result;
use cargo_metadata::diagnostic::Diagnostic;
use dioxus_cli_config::Platform;
use futures_util::Future;
use manganis_cli_support::AssetManifest;
use std::cell::RefCell;
use std::{path::PathBuf, time::Duration};
use tokio::process::Child;
use tokio::sync::RwLock;
use tokio::task::{JoinHandle, JoinSet};

/// A handle to ongoing builds and then the spawned tasks themselves
pub struct Builder {
    /// The results of the build
    build_results: Option<JoinHandle<Result<Vec<BuildResult>>>>,
    /// The application we are building
    config: DioxusCrate,
    /// The arguments for the build
    build_arguments: Build,
}

impl Builder {
    /// Create a new builder
    pub fn new(config: &DioxusCrate, serve: &Serve) -> Self {
        let config = config.clone();
        let build_arguments = serve.build_arguments.clone();
        Self {
            build_results: None,
            config,
            build_arguments,
        }
    }

    /// Start a new build - killing the current one if it exists
    pub async fn build(&mut self) -> Result<()> {
        self.shutdown().await?;
        let build_requests =
            BuildRequest::create(false, self.config.clone(), self.build_arguments.clone());

        let mut set = tokio::task::JoinSet::new();
        for build_request in build_requests {
            set.spawn(async move { build_request.build().await });
        }

        self.build_results = Some(tokio::spawn({
            async move {
                let mut all_results = Vec::new();
                while let Some(result) = set.join_next().await {
                    all_results.push(result.map_err(|err| {
                        crate::Error::Unique(format!("Panic while building project: {err:?}"))
                    })??);
                }
                Ok(all_results)
            }
        }));

        Ok(())
    }

    /// Wait for any new updates to the builder - either it completed or gave us a message etc
    pub async fn wait(&mut self) -> Result<Vec<BuildResult>> {
        if let Some(tasks) = self.build_results.take() {
            tasks
                .await
                .map_err(|_| crate::Error::BuildFailed("Build failed".to_string()))?
        } else {
            std::future::pending().await
        }
    }

    /// Shutdown the current build process
    pub(crate) async fn shutdown(&mut self) -> Result<()> {
        if let Some(tasks) = self.build_results.take() {
            tasks.abort();
        }
        Ok(())
    }
}
