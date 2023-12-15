use clap::Parser;
use ipnet::IpNet;
use reqwest::{self, header, Error as ReqwestError};
use std::{
    fs::File,
    io::Write,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinSet;

const CDN_DOMAIN: &str = "v2ex.com";
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    target: IpNet,
    #[arg(long, default_value = "3")]
    timeout: u64,
    #[arg(short, long, default_value = "5")]
    concurrency: usize,
}

async fn check_if_cf_proxy(ip: IpAddr, timeout: u64) -> Result<bool, ReqwestError> {
    let client = reqwest::Client::builder()
        .resolve(CDN_DOMAIN, SocketAddr::new(ip, 443))
        .timeout(Duration::from_secs(timeout))
        .danger_accept_invalid_certs(true)
        .build()?;

    let res = client
        .get(format!("https://{}/cdn-cgi/trace", CDN_DOMAIN)) // TODO check both http and https
        .header(header::HOST, CDN_DOMAIN)
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
    let total_ips = args.target.hosts().count();
    let proxy_ips: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut checked_ips = 0;
    println!("Checking {} IPs", total_ips);
    let mut task_set = JoinSet::new();
    for ip in args.target.hosts() {
        checked_ips += 1;
        println!("Starting to check {} ({}/{})", ip, checked_ips, total_ips);
        let proxy_ips_clone = Arc::clone(&proxy_ips);
        task_set.spawn(async move {
            match check_if_cf_proxy(ip, args.timeout).await {
                Ok(true) => {
                    println!("{} is a cloudflare proxy", ip);
                    let mut proxy_ips = proxy_ips_clone.lock().unwrap();
                    proxy_ips.push(ip.to_string());
                }
                Ok(false) => {
                    println!("{} is not a cloudflare proxy", ip);
                }
                // 忽略ReqwestError错误
                Err(_) => {
                    println!("{} is not a cloudflare proxy", ip);
                }
            }
        });
        if task_set.len() >= args.concurrency || checked_ips == total_ips {
            let _ = task_set.join_next().await.expect("task failed");
        }
    }

    let proxy_ips = proxy_ips.lock().unwrap();
    println!(
        "Checked {} IPs, found {} proxy ip",
        proxy_ips.len(),
        args.target.hosts().count(),
    );
    let mut file = File::create("proxy_ips.txt").unwrap();
    file.write_all(proxy_ips.join("\n").as_bytes()).unwrap();
}
