use std::{net::Ipv4Addr, path::PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use once_cell::sync::Lazy;
use reqwest::Client;
use tracing::debug;

const LOGIN_PORTAL_URL: &str = "https://login.ruhr-uni-bochum.de/cgi-bin/start";
const LOGIN_URL: &str = "https://login.ruhr-uni-bochum.de/cgi-bin/laklogin";

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    FindIp(FindIp),
    Login(Login),
    Logout(Logout),
}

#[derive(Parser, Debug)]
struct FindIp;

#[derive(Parser, Debug)]
struct Login {
    /// Ip to authenticate. If no ip is provided, rub-login will try to determine the local ip.
    #[clap(long)]
    ip: Option<Ipv4Addr>,
    #[clap(value_parser)]
    username: String,
    #[clap(value_parser = clap::value_parser!(PathBuf))]
    password_file: PathBuf,
}

#[derive(Parser, Debug)]
struct Logout {
    /// Ip to authenticate. If no ip is provided, rub-login will try to determine the local ip.
    #[clap(long)]
    ip: Option<Ipv4Addr>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    match &args.command {
        Command::FindIp(find_ip) => find_ip.run().await,
        Command::Login(login) => login.run().await,
        Command::Logout(login) => login.run().await,
    }
}

impl FindIp {
    async fn run(&self) -> Result<()> {
        let ip = find_local_ip()
            .await
            .context("failed to determine local ip")?;
        if let Some(ip) = ip {
            println!("Local ip: {ip}");
        } else {
            println!("You're not inside the HIRN!");
        }
        Ok(())
    }
}

impl Login {
    async fn run(&self) -> Result<()> {
        let password =
            std::fs::read_to_string(&self.password_file).context("failed to read password file")?;

        let ip = if let Some(ip) = self.ip {
            ip
        } else {
            let ip = find_local_ip()
                .await
                .context("failed to determine local ip")?;
            if let Some(ip) = ip {
                ip
            } else {
                println!("Not inside HIRN.");
                return Ok(());
            }
        };

        login(&self.username, &password, ip)
            .await
            .context("failed to log in")?;

        Ok(())
    }
}

impl Logout {
    async fn run(&self) -> Result<()> {
        let ip = if let Some(ip) = self.ip {
            ip
        } else {
            let ip = find_local_ip()
                .await
                .context("failed to determine local ip")?;
            if let Some(ip) = ip {
                ip
            } else {
                println!("Not inside HIRN.");
                return Ok(());
            }
        };

        logout(ip).await.context("failed to log out")?;

        Ok(())
    }
}

/// Determine the local ip in HIRN.
async fn find_local_ip() -> Result<Option<Ipv4Addr>> {
    let resp = CLIENT
        .get(LOGIN_PORTAL_URL)
        .send()
        .await
        .context("failed to send request")?;
    let text = resp.text().await.context("failed to read response")?;

    // Check if we made the request from inside HIRN.
    if text.contains("befinden sich an einem Arbeitsplatz der nicht Lock-And-Key") {
        debug!("We're not inside HIRN");
        return Ok(None);
    }

    // Extract the ip from the response.
    let start_needle = r#"name="ipaddr" value=""#;
    let end_needle = '"';

    let start_index = text
        .find(start_needle)
        .context("failed to extract ip from response")?;
    let value_index = start_index + start_needle.len();
    let value = &text[value_index..];
    let end_index = value
        .find(end_needle)
        .context("failed to extract ip from response")?;
    let value = &value[..end_index];

    // Parse the ip.
    let ip = value.parse().context("failed to parse the ip")?;
    debug!(%ip, "Determined local ip");
    Ok(Some(ip))
}

async fn login(username: &str, password: &str, ip: Ipv4Addr) -> Result<()> {
    debug!(username, %ip, "Logging in");

    let form = (
        ("code", 1),
        ("loginid", username),
        ("password", password),
        ("ipaddr", ip),
        ("action", "Login"),
    );
    let resp = CLIENT
        .post(LOGIN_URL)
        .form(&form)
        .send()
        .await
        .context("failed to send request")?;
    let text = resp.text().await.context("failed to read response")?;

    // Check for success.
    if text.contains("Authentisierung gelungen") {
        return Ok(());
    }

    // Check for failure.
    if text.contains("Authentisierung fehlgeschlagen") {
        bail!("Authentication failed");
    }

    // Otherwise something has gone wrong.
    bail!("Unexpected response")
}

async fn logout(ip: Ipv4Addr) -> Result<()> {
    let form = (
        ("code", 1),
        ("loginid", ""),
        ("password", ""),
        ("ipaddr", ip),
        ("action", "Logout"),
    );
    let resp = CLIENT
        .post(LOGIN_URL)
        .form(&form)
        .send()
        .await
        .context("failed to send request")?;
    let text = resp.text().await.context("failed to read response")?;

    // Check for success.
    if text.contains("Logout erfolgreich") {
        return Ok(());
    }

    // Check for failure.
    // The error message is a bit misleading.
    if text.contains("Authentisierung fehlgeschlagen") {
        bail!("Authentication failed");
    }

    // Otherwise something has gone wrong.
    bail!("Unexpected response")
}
