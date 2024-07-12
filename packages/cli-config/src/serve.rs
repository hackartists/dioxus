use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use clap::Parser;

/// The arguments for the address the server will run on
#[derive(Clone, Debug, Parser)]
pub struct AddressArguments {
    /// The port the server will run on
    #[clap(long)]
    #[clap(default_value_t = default_port())]
    pub port: u16,

    /// The address the server will run on
    #[clap(long, default_value_t = default_address())]
    pub addr: std::net::IpAddr,
}

impl Default for AddressArguments {
    fn default() -> Self {
        Self {
            port: default_port(),
            addr: default_address(),
        }
    }
}

impl AddressArguments {
    /// Get the address the server should run on
    pub fn address(&self) -> SocketAddr {
        SocketAddr::new(self.addr, self.port)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RuntimeCLIArguments {
    /// The address hot reloading is running on
    pub cli_address: SocketAddr,

    /// The address the server should run on
    pub server_socket: Option<SocketAddr>,
}

impl RuntimeCLIArguments {
    /// Attempt to read the current serve settings from the CLI. This will only be set for the fullstack platform on recent versions of the CLI.
    pub fn from_cli() -> Option<Self> {
        std::env::var(crate::__private::SERVE_ENV)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
    }

    /// Get the address the server should run on
    pub fn server_socket(&self) -> Option<SocketAddr> {
        self.server_socket
    }
}

impl From<RuntimeCLIArguments> for AddressArguments {
    fn from(args: RuntimeCLIArguments) -> Self {
        let socket = args.server_socket.unwrap_or_else(|| {
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, default_port()))
        });

        Self {
            port: socket.port(),
            addr: socket.ip(),
        }
    }
}

fn default_port() -> u16 {
    8080
}

fn default_address() -> IpAddr {
    IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))
    // IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))
}
