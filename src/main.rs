use clap::Parser;
use ipnet::IpNet;
use reqwest::{self, header, Error as ReqwestError};
use std::{fs::File, io::Write, net::IpAddr, time::Duration};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    target: IpNet,
    #[arg(long, default_value = "3")]
    timeout: u64,
}

async fn check_if_cf_proxy(ip: IpAddr, timeout: u64) -> Result<bool, ReqwestError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .danger_accept_invalid_certs(true)
        .build()?;

    let res = client
        .get(format!("http://{}/cdn-cgi/trace", ip)) // TODO check both http and https
        .header(header::HOST, "v2ex.com")
        .send()
        .await?;
    let response_text = res.text().await?;
    // println!("{}", response_text);
    if response_text.contains("h=v2ex.com") {
        return Ok(true);
    }
    Ok(false)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut proxy_ips: Vec<String> = Vec::new();
    let total_ips = args.target.hosts().count();
    let mut checked_ips = 0;
    println!("Checking {} IPs", total_ips);
    for ip in args.target.hosts() {
        checked_ips += 1;
        println!("Starting to check {} ({}/{})", ip, checked_ips, total_ips);
        match check_if_cf_proxy(ip, args.timeout).await {
            Ok(true) => {
                println!("{} is behind CDN", ip);
                proxy_ips.push(ip.to_string());
            }
            Ok(false) => {
                println!("{} is not behind CDN", ip);
            }
            // 忽略ReqwestError错误
            Err(_) => {
                println!("{} is not behind CDN", ip);
            }
        }
    }
    println!(
        "Checked {} IPs, found {} proxy ip",
        args.target.hosts().count(),
        proxy_ips.len()
    );
    let mut file = File::create("proxy_ips.txt").unwrap();
    file.write_all(proxy_ips.join("\n").as_bytes()).unwrap();
}
