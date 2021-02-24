use anyhow::Result;
use clap::{App, Arg};
use socksx::{self, Socks5Client, Socks6Client};
use tokio::net::{TcpListener, TcpStream};

// iptables -t nat -A OUTPUT ! -d $PROXY_HOST/32 -o eth0 -p tcp -m tcp -j REDIRECT --to-ports 42000

#[tokio::main]
async fn main() -> Result<()> {
    let args = App::new("Redirector")
        .arg(
            Arg::with_name("VERSION")
                .short("s")
                .long("socks")
                .help("The SOCKS version to use")
                .default_value("5"),
        )
        .arg(
            Arg::with_name("PROXY")
                .help("The IP or hostname of the proxy")
                .required(true),
        )
        .get_matches();

    let proxy_host = format!("{}:1080", args.value_of("PROXY").unwrap_or("127.0.0.1"));
    let listener = TcpListener::bind("127.0.0.1:42000").await?;

    match args.value_of("VERSION") {
        Some("5") => {
            let client = Socks5Client::new(proxy_host, None).await?;

            loop {
                let (stream, _) = listener.accept().await?;
                tokio::spawn(redirect_v5(stream, client.clone()));
            }
        }
        Some("6") => {
            let client = Socks6Client::new(proxy_host, None).await?;

            loop {
                let (stream, _) = listener.accept().await?;
                tokio::spawn(redirect_v6(stream, client.clone()));
            }
        }
        Some(version) => panic!("Unsupported version: {}", version),
        None => unreachable!(),
    };
}

/// Redirect an incoming TCP stream through a SOCKS5
/// proxy. The original destination of the stream has
/// been preserved, by iptables, as an socket option.
async fn redirect_v5(
    incoming: TcpStream,
    client: Socks5Client,
) -> Result<()> {
    let mut incoming = incoming;

    let dst_addr = socksx::get_original_dst(&incoming)?;
    let (mut outgoing, _) = client.connect(dst_addr).await?;

    socksx::bidirectional_copy(&mut incoming, &mut outgoing).await?;

    Ok(())
}

/// Redirect an incoming TCP stream through a SOCKS6
/// proxy. The original destination of the stream has
/// been preserved, by iptables, as an socket option.
async fn redirect_v6(
    incoming: TcpStream,
    client: Socks6Client,
) -> Result<()> {
    let mut incoming = incoming;

    let dst_addr = socksx::get_original_dst(&incoming)?;
    let initial_data = socksx::try_read_initial_data(&mut incoming).await?;
    let (mut outgoing, _) = client.connect(dst_addr, initial_data, None).await?;

    socksx::bidirectional_copy(&mut incoming, &mut outgoing).await?;

    Ok(())
}
