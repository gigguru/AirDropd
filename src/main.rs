#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![warn(unused_imports, unused_mut)]
#![allow(dead_code, mismatched_lifetime_syntaxes)]

use std::sync::Arc;
use tokio::sync::Mutex;

mod config;
mod network;
mod protocols;
mod ui;
mod utils;

use config::{shared, AppConfig};
use network::discovery::DeviceDiscovery;
use network::ble::BleManager;
use protocols::airdrop::AirDrop;
use protocols::airplay::AirPlay;
use protocols::awdl::{AwdlManager, AwdlManagerConfig};
use std::path::PathBuf;

/// Core background services for AirDropd.
pub struct AirDropdServices {
    pub config: config::SharedConfig,
    pub device_discovery: Arc<Mutex<DeviceDiscovery>>,
    pub airdrop: Arc<Mutex<AirDrop>>,
    pub airplay: Arc<Mutex<AirPlay>>,
    pub ble: Arc<Mutex<BleManager>>,
    pub awdl: Arc<Mutex<AwdlManager>>,
    pub received_tx: tokio::sync::broadcast::Sender<PathBuf>,
}

impl AirDropdServices {
    pub async fn new(app_config: config::SharedConfig) -> anyhow::Result<Self> {
        let (received_tx, _) = tokio::sync::broadcast::channel(16);

        let discovery = DeviceDiscovery::new()?;
        let airdrop = AirDrop::new(app_config.clone(), received_tx.clone());
        let airplay = AirPlay::new();
        let ble = BleManager::new().await?;
        let awdl = AwdlManager::new(AwdlManagerConfig::default());

        Ok(Self {
            config: app_config,
            device_discovery: Arc::new(Mutex::new(discovery)),
            airdrop: Arc::new(Mutex::new(airdrop)),
            airplay: Arc::new(Mutex::new(airplay)),
            ble: Arc::new(Mutex::new(ble)),
            awdl: Arc::new(Mutex::new(awdl)),
            received_tx,
        })
    }

    pub async fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let discovery = self.device_discovery.lock().await;
            discovery.start_discovery().await?;
        }

        {
            let airdrop = self.airdrop.lock().await;
            airdrop.start_server().await?;
        }

        {
            let airplay = self.airplay.lock().await;
            airplay.start_server().await?;
        }

        {
            let broadcast_name = self
                .config
                .read()
                .map(|c| c.broadcast_name.clone())
                .unwrap_or_else(|_| config::default_broadcast_name());

            let mut ble = self.ble.lock().await;
            ble.initialize().await?;
            ble.start_advertising_with_name(&broadcast_name).await?;
        }

        {
            let mut awdl = self.awdl.lock().await;
            awdl.initialize().await?;
        }

        {
            let awdl = self.awdl.lock().await;
            awdl.refresh_peers().await;
        }

        Ok(())
    }

    /// Apply saved settings to live discovery services (BLE name, etc.).
    pub async fn apply_settings(&self) -> anyhow::Result<()> {
        let broadcast_name = self
            .config
            .read()
            .map(|c| c.broadcast_name.clone())
            .unwrap_or_else(|_| config::default_broadcast_name());

        {
            let ble = self.ble.lock().await;
            ble.restart_advertising(&broadcast_name).await?;
        }

        Ok(())
    }
}

pub async fn awdl_peer_refresh_loop(services: Arc<AirDropdServices>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
    loop {
        interval.tick().await;
        let awdl = services.awdl.lock().await;
        awdl.refresh_peers().await;
    }
}

fn init_logging() {
    #[cfg(debug_assertions)]
    {
        let _ = env_logger::try_init();
    }

    #[cfg(not(debug_assertions))]
    {
        use tracing_subscriber::EnvFilter;
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("airdropd=info")),
            )
            .with_writer(std::io::sink)
            .try_init();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let cfg = AppConfig::load();
    let app_config = shared(cfg);

    let runtime = tokio::runtime::Runtime::new()?;

    let services = runtime.block_on(async {
        match AirDropdServices::new(app_config).await {
            Ok(s) => Arc::new(s),
            Err(e) => {
                tracing::error!("Error creating services: {}", e);
                std::process::exit(1);
            }
        }
    });

    let services_clone = services.clone();
    runtime.spawn(async move {
        if let Err(e) = services_clone.initialize().await {
            tracing::error!("Error initializing services: {}", e);
        }
    });

    let awdl_refresh = services.clone();
    runtime.spawn(async move {
        awdl_peer_refresh_loop(awdl_refresh).await;
    });

    std::thread::spawn(move || {
        runtime.block_on(async {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });
    });

    ui::run(services)?;

    Ok(())
}
