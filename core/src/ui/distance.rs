//! Rough distance estimates from BLE signal strength, shown in feet.

/// Typical RSSI (dBm) at ~1 m for consumer BLE beacons (calibrated for indoor).
const TX_POWER_DBM: f32 = -57.0;
const PATH_LOSS_EXPONENT: f32 = 2.35;
const METERS_TO_FEET: f32 = 3.280_84;

/// Sonar ring labels — must stay in sync with `radar.rs` ring drawing.
pub const SONAR_RING_FEET: [f32; 3] = [8.0, 20.0, 45.0];
pub const SONAR_RING_FRACTIONS: [f32; 3] = [0.33, 0.66, 0.92];
const SONAR_MIN_FEET: f32 = 1.0;
const SONAR_MAX_FEET: f32 = 50.0;

/// Estimate range in feet from RSSI using a log-distance path-loss model.
pub fn rssi_to_feet(rssi: i16) -> u32 {
    let exponent = (TX_POWER_DBM - rssi as f32) / (10.0 * PATH_LOSS_EXPONENT);
    let meters = 10_f32.powf(exponent).clamp(0.25, 36.0);
    ((meters * METERS_TO_FEET).round() as u32).clamp(1, 50)
}

/// Map estimated feet to a radar radius fraction aligned with the 8' / 20' / 45' rings.
pub fn feet_to_radius_fraction(feet: f32) -> f32 {
    let feet = feet.clamp(SONAR_MIN_FEET, SONAR_MAX_FEET);
    if feet <= SONAR_RING_FEET[0] {
        let span = SONAR_RING_FEET[0] - SONAR_MIN_FEET;
        let t = if span > 0.0 {
            (feet - SONAR_MIN_FEET) / span
        } else {
            0.0
        };
        return 0.12 + t * (SONAR_RING_FRACTIONS[0] - 0.12);
    }
    if feet <= SONAR_RING_FEET[1] {
        let span = SONAR_RING_FEET[1] - SONAR_RING_FEET[0];
        let t = (feet - SONAR_RING_FEET[0]) / span;
        return SONAR_RING_FRACTIONS[0] + t * (SONAR_RING_FRACTIONS[1] - SONAR_RING_FRACTIONS[0]);
    }
    if feet <= SONAR_RING_FEET[2] {
        let span = SONAR_RING_FEET[2] - SONAR_RING_FEET[1];
        let t = (feet - SONAR_RING_FEET[1]) / span;
        return SONAR_RING_FRACTIONS[1] + t * (SONAR_RING_FRACTIONS[2] - SONAR_RING_FRACTIONS[1]);
    }
    SONAR_RING_FRACTIONS[2]
}

/// Human-friendly feet label, e.g. `5'` or `24 ft`.
pub fn format_feet(feet: u32) -> String {
    if feet < 10 {
        format!("{feet}'")
    } else {
        format!("{feet} ft")
    }
}

/// Optional distance line for a device with BLE signal.
pub fn device_distance_label(rssi: Option<i16>) -> Option<String> {
    rssi.map(|r| format_feet(rssi_to_feet(r)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_markers_map_to_ring_fractions() {
        assert!((feet_to_radius_fraction(8.0) - 0.33).abs() < 0.01);
        assert!((feet_to_radius_fraction(20.0) - 0.66).abs() < 0.01);
        assert!((feet_to_radius_fraction(45.0) - 0.92).abs() < 0.01);
    }

    #[test]
    fn strong_signal_is_close() {
        assert!(rssi_to_feet(-55) <= 5);
        assert!(rssi_to_feet(-80) >= 15);
    }
}
