//! License and weekly demo usage persisted in [`crate::config::AppConfig`].

use chrono::{Datelike, Utc};

use super::fingerprint::machine_fingerprint;
use super::key::{format_product_key, normalize_product_key, validate_product_key};
use super::limits::{
    LicenseStatus, MAX_ACTIVATIONS_PER_KEY, DEMO_MAX_FILE_BYTES, DEMO_QR_UPLOADS_PER_WEEK,
    DEMO_SENDS_PER_WEEK,
};
use super::KEY_SECRET;

/// Licensing fields stored inside application config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LicenseFields {
    pub license_key: Option<String>,
    pub license_fingerprints: Vec<String>,
    pub demo_week_id: String,
    pub demo_sends_this_week: u32,
    pub demo_qr_uploads_this_week: u32,
}

impl Default for LicenseFields {
    fn default() -> Self {
        Self {
            license_key: None,
            license_fingerprints: Vec::new(),
            demo_week_id: current_week_id(),
            demo_sends_this_week: 0,
            demo_qr_uploads_this_week: 0,
        }
    }
}

pub struct LicenseStore<'a> {
    fields: &'a mut LicenseFields,
}

impl<'a> LicenseStore<'a> {
    pub fn new(fields: &'a mut LicenseFields) -> Self {
        Self { fields }
    }

    pub fn status(&self) -> LicenseStatus {
        if self
            .fields
            .license_key
            .as_ref()
            .is_some_and(|k| validate_product_key(k, KEY_SECRET).is_ok())
        {
            LicenseStatus::Registered
        } else {
            LicenseStatus::Demo
        }
    }

    pub fn is_registered(&self) -> bool {
        self.status() == LicenseStatus::Registered
    }

    pub fn formatted_key(&self) -> Option<String> {
        self.fields
            .license_key
            .as_ref()
            .map(|k| format_product_key(k))
    }

    pub fn demo_sends_remaining(&mut self) -> u32 {
        self.roll_week_if_needed();
        DEMO_SENDS_PER_WEEK.saturating_sub(self.fields.demo_sends_this_week)
    }

    pub fn demo_qr_remaining(&mut self) -> u32 {
        self.roll_week_if_needed();
        DEMO_QR_UPLOADS_PER_WEEK.saturating_sub(self.fields.demo_qr_uploads_this_week)
    }

    pub fn record_demo_send(&mut self) {
        self.roll_week_if_needed();
        self.fields.demo_sends_this_week = self.fields.demo_sends_this_week.saturating_add(1);
    }

    pub fn record_demo_qr_upload(&mut self) {
        self.roll_week_if_needed();
        self.fields.demo_qr_uploads_this_week =
            self.fields.demo_qr_uploads_this_week.saturating_add(1);
    }

    pub fn activate(&mut self, key: &str) -> Result<(), ActivationError> {
        validate_product_key(key, KEY_SECRET).map_err(|_| ActivationError::InvalidKey)?;
        let formatted = format_product_key(key);
        let fp = machine_fingerprint();

        if let Some(existing) = &self.fields.license_key {
            let same = normalize_product_key(existing) == normalize_product_key(&formatted);
            if same && self.fields.license_fingerprints.contains(&fp) {
                return Ok(());
            }
            if same && self.fields.license_fingerprints.len() >= MAX_ACTIVATIONS_PER_KEY {
                return Err(ActivationError::ActivationLimitReached);
            }
            if !same {
                self.fields.license_fingerprints.clear();
            }
        }

        if !self.fields.license_fingerprints.contains(&fp) {
            if self.fields.license_fingerprints.len() >= MAX_ACTIVATIONS_PER_KEY {
                return Err(ActivationError::ActivationLimitReached);
            }
            self.fields.license_fingerprints.push(fp);
        }

        self.fields.license_key = Some(formatted);
        Ok(())
    }

    pub fn deactivate(&mut self) {
        let fp = machine_fingerprint();
        self.fields.license_fingerprints.retain(|f| f != &fp);
        if self.fields.license_fingerprints.is_empty() {
            self.fields.license_key = None;
        }
    }

    pub fn deactivate_all(&mut self) {
        self.fields.license_key = None;
        self.fields.license_fingerprints.clear();
    }

    pub fn check_send(&mut self, total_bytes: u64, item_count: usize) -> Result<(), DemoLimitError> {
        if self.is_registered() {
            return Ok(());
        }
        if total_bytes > DEMO_MAX_FILE_BYTES {
            return Err(DemoLimitError::FileTooLarge {
                max_mb: DEMO_MAX_FILE_BYTES / (1024 * 1024),
            });
        }
        if item_count > 1 {
            return Err(DemoLimitError::FoldersRequireRegistration);
        }
        self.roll_week_if_needed();
        if self.fields.demo_sends_this_week >= DEMO_SENDS_PER_WEEK {
            return Err(DemoLimitError::WeeklySendLimit {
                limit: DEMO_SENDS_PER_WEEK,
            });
        }
        Ok(())
    }

    pub fn check_qr_upload(&mut self, file_bytes: u64) -> Result<(), DemoLimitError> {
        if self.is_registered() {
            return Ok(());
        }
        if file_bytes > DEMO_MAX_FILE_BYTES {
            return Err(DemoLimitError::FileTooLarge {
                max_mb: DEMO_MAX_FILE_BYTES / (1024 * 1024),
            });
        }
        self.roll_week_if_needed();
        if self.fields.demo_qr_uploads_this_week >= DEMO_QR_UPLOADS_PER_WEEK {
            return Err(DemoLimitError::WeeklyQrLimit {
                limit: DEMO_QR_UPLOADS_PER_WEEK,
            });
        }
        Ok(())
    }

    pub fn requires_registration_for_feature(&self, feature: GatedFeature) -> bool {
        if self.is_registered() {
            return false;
        }
        matches!(
            feature,
            GatedFeature::AutoAccept
                | GatedFeature::ShowAllDevices
                | GatedFeature::AdvancedDiscovery
                | GatedFeature::DiscoveryFreeze
        )
    }

    fn roll_week_if_needed(&mut self) {
        let week = current_week_id();
        if self.fields.demo_week_id != week {
            self.fields.demo_week_id = week;
            self.fields.demo_sends_this_week = 0;
            self.fields.demo_qr_uploads_this_week = 0;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatedFeature {
    AutoAccept,
    ShowAllDevices,
    AdvancedDiscovery,
    DiscoveryFreeze,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationError {
    InvalidKey,
    ActivationLimitReached,
}

impl std::fmt::Display for ActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidKey => write!(f, "That product key is not valid."),
            Self::ActivationLimitReached => write!(
                f,
                "This key is already active on {MAX_ACTIVATIONS_PER_KEY} computers. \
                 Deactivate AirDropd on another device in Settings → Registration, then try again."
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoLimitError {
    WeeklySendLimit { limit: u32 },
    WeeklyQrLimit { limit: u32 },
    FileTooLarge { max_mb: u64 },
    FoldersRequireRegistration,
}

impl std::fmt::Display for DemoLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WeeklySendLimit { limit } => write!(
                f,
                "Demo limit reached ({limit} AirDrop sends this week). Register with a product key for unlimited transfers."
            ),
            Self::WeeklyQrLimit { limit } => write!(
                f,
                "Demo limit reached ({limit} QR uploads this week). Register for unlimited Web Drop and DJ Mode uploads."
            ),
            Self::FileTooLarge { max_mb } => write!(
                f,
                "Demo file size limit is {max_mb} MB. Register for larger transfers."
            ),
            Self::FoldersRequireRegistration => write!(
                f,
                "Sending folders requires registration. Register with your product key in Settings."
            ),
        }
    }
}

pub fn current_week_id() -> String {
    let now = Utc::now();
    format!("{}-W{:02}", now.year(), now.iso_week().week())
}
