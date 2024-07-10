use crate::dioxus_crate::DioxusCrate;
use crate::serve::Serve;
use crate::Result;
use tokio::task::yield_now;

mod builder;
mod hot_reloading_file_map;
mod output;
mod proxy;
mod server;
mod watcher;

use builder::*;
use output::*;
use server::*;
use watcher::*;

/// For *all* builds the CLI spins up a dedicated webserver, file watcher, and build infrastructure to serve the project.
///
/// This includes web, desktop, mobile, fullstack, etc.
///
/// Platform specifics:
/// - Web:       we need to attach a filesystem server to our devtools webserver to serve the project. We
///              want to emulate GithubPages here since most folks are deploying there and expect things like
///              basepath to match.
/// - Fullstack: We spin up the same dev server but in this case the fullstack server itself needs to
///              proxy all dev requests to our dev server
/// - Desktop:   We spin up the dev server but without a filesystem server.
/// - Mobile:    Basically the same as desktop.
///
/// Notes:
/// - All filesystem changes are tracked here
/// - We send all updates to connected websocket connections. Even desktop connects via the websocket
/// - Right now desktop compiles tokio-tungstenite to do the connection but we could in theory reuse
///   the websocket logic from the webview for thinner builds.
///
/// Todos(Jon):
/// - I'd love to be able to configure the CLI while it's running so we can change settingaon the fly.
///   This would require some light refactoring and potentially pulling in something like ratatui.
/// - Build a custom subscriber for logs by tools within this
/// - Handle logs from the build engine separately?
/// - Consume logs from the wasm for web/fullstack
/// - I want us to be able to detect a `server_fn` in the project and then upgrade from a static server
///   to a dynamic one on the fly.
pub async fn serve_all(serve: Serve, dioxus_crate: DioxusCrate) -> Result<()> {
    let mut server = Server::start(&serve, &dioxus_crate).await;
    let mut watchr = Watcher::start(&dioxus_crate);
    let mut screen = Output::start(&serve, &dioxus_crate)
        .await
        .expect("Failed to open terminal logger");
    let mut buildr = Builder::new(&dioxus_crate, &serve);

    // Start the first build
    buildr.build();

    loop {
        // Make sure we don't hog the CPU: these loop { select! {} } blocks can starve the executor
        yield_now().await;

        // Draw the state of the server to the screen
        screen.draw(&serve, &dioxus_crate, &buildr, &server, &watchr);

        // Also update the webserver page if we need to
        // This will send updates about the current status of the build
        server.update(&serve, &dioxus_crate);

        // And then wait for any updates before redrawing
        tokio::select! {
            // rebuild the project or hotreload it
            _ = watchr.wait() => {
                if !watchr.pending_changes() {
                    continue;
                }

                println!("Changes detected! Reloading...");

                // if change is hotreloadable, hotreload it
                // and then send that update to all connected clients
                if let Some(hr) = watchr.attempt_hot_reload(&dioxus_crate) {
                    println!("Hotreloading... {hr:?}");
                    server.send_hotreload(hr).await;
                    continue;
                }

                // If the change is not binary patchable, rebuild the project
                // We're going to kick off a new build, interrupting the current build if it's ongoing
                buildr.build();
            }

            // reload the pag
            _new_socket = server.wait() => {
                // Run the server in the background
                // Waiting for updates here lets us tap into when clients are added/removed
            }

            // Handle updates from the build engine
            application = buildr.wait() => {
                // Wait for logs from the build engine
                // These will cause us to update the screen
                // We also can check the status of the builds here in case we have multiple ongoing builds
                if let Some((platform, update)) = application? {
                    // println!("Update for {platform:?}...");
                }
            }

            // Handle input from the user using our settings
            input = screen.wait() => {
                if screen.handle_input(input).is_err() {
                    break;
                }
            }
        }
    }

    // Run our cleanup logic here - maybe printing as we go?
    // todo: more printing, logging, error handling in this phase
    _ = screen.shutdown();
    _ = server.shutdown().await;
    _ = buildr.shutdown();

    Ok(())
}
