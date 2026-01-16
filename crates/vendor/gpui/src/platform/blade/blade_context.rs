use anyhow::Context as _;
use blade_graphics as gpu;
use std::sync::Arc;
use std::time::Duration;
use util::ResultExt;

#[cfg_attr(target_os = "macos", derive(Clone))]
pub struct BladeContext {
    pub(super) gpu: Arc<gpu::Context>,
}

impl BladeContext {
    pub fn new() -> anyhow::Result<Self> {
        let device_id_forced = match std::env::var("ZED_DEVICE_ID") {
            Ok(val) => parse_pci_id(&val)
                .context("Failed to parse device ID from `ZED_DEVICE_ID` environment variable")
                .log_err(),
            Err(std::env::VarError::NotPresent) => None,
            err => {
                err.context("Failed to read value of `ZED_DEVICE_ID` environment variable")
                    .log_err();
                None
            }
        };
        let device_id = device_id_forced.unwrap_or(0);
        let gpu = Arc::new(init_with_retry(|| {
            unsafe {
                gpu::Context::init(gpu::ContextDesc {
                    presentation: true,
                    validation: false,
                    device_id,
                    ..Default::default()
                })
            }
            .map_err(|e| format!("{e:?}"))
        })?);
        Ok(Self { gpu })
    }
}

fn is_device_lost_error(err: &str) -> bool {
    err.contains("ERROR_DEVICE_LOST") || err.contains("DeviceLost") || err.contains("DEVICE_LOST")
}

fn init_with_retry<T>(mut init: impl FnMut() -> Result<T, String>) -> anyhow::Result<T> {
    init_with_retry_and_sleep(&mut init, |d| std::thread::sleep(d))
}

fn init_with_retry_and_sleep<T>(
    init: &mut impl FnMut() -> Result<T, String>,
    mut sleep: impl FnMut(Duration),
) -> anyhow::Result<T> {
    // Device-lost during adapter/device init can be transient (e.g. after a GPU reset). Retry a
    // few times with a small backoff before giving up.
    const RETRY_DELAYS: [Duration; 3] = [
        Duration::from_millis(50),
        Duration::from_millis(200),
        Duration::from_millis(500),
    ];

    for (attempt, delay) in RETRY_DELAYS.iter().enumerate() {
        match init() {
            Ok(val) => return Ok(val),
            Err(err) if is_device_lost_error(&err) => {
                sleep(*delay);
                log::error!(
                    "GPU init failed with device-lost (attempt {} of {}): {}",
                    attempt + 1,
                    RETRY_DELAYS.len() + 1,
                    err
                );
                continue;
            }
            Err(err) => return Err(anyhow::anyhow!("{err}")),
        }
    }

    // Final attempt (no delay after).
    init().map_err(|err| anyhow::anyhow!("{err}"))
}

fn parse_pci_id(id: &str) -> anyhow::Result<u32> {
    let mut id = id.trim();

    if id.starts_with("0x") || id.starts_with("0X") {
        id = &id[2..];
    }
    let is_hex_string = id.chars().all(|c| c.is_ascii_hexdigit());
    let is_4_chars = id.len() == 4;
    anyhow::ensure!(
        is_4_chars && is_hex_string,
        "Expected a 4 digit PCI ID in hexadecimal format"
    );

    u32::from_str_radix(id, 16).context("parsing PCI ID as hex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_device_id() {
        assert!(parse_pci_id("0xABCD").is_ok());
        assert!(parse_pci_id("ABCD").is_ok());
        assert!(parse_pci_id("abcd").is_ok());
        assert!(parse_pci_id("1234").is_ok());
        assert!(parse_pci_id("123").is_err());
        assert_eq!(
            parse_pci_id(&format!("{:x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:X}", 0x1234)).unwrap(),
        );

        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
        assert_eq!(
            parse_pci_id(&format!("{:#x}", 0x1234)).unwrap(),
            parse_pci_id(&format!("{:#X}", 0x1234)).unwrap(),
        );
    }

    #[test]
    fn retries_device_lost_then_succeeds() {
        let mut attempts = 0usize;
        let mut slept = 0usize;
        let mut init = || {
            attempts += 1;
            if attempts < 3 {
                Err("Platform(Init(ERROR_DEVICE_LOST))".to_string())
            } else {
                Ok(42)
            }
        };

        let out = init_with_retry_and_sleep(&mut init, |_d| slept += 1).unwrap();
        assert_eq!(out, 42);
        assert_eq!(attempts, 3);
        assert_eq!(slept, 2);
    }

    #[test]
    fn does_not_retry_non_device_lost() {
        let mut attempts = 0usize;
        let mut slept = 0usize;
        let mut init = || -> Result<i32, String> {
            attempts += 1;
            Err("OtherError".to_string())
        };

        assert!(init_with_retry_and_sleep(&mut init, |_d| slept += 1).is_err());
        assert_eq!(attempts, 1);
        assert_eq!(slept, 0);
    }

    #[test]
    fn gives_up_after_retries() {
        let mut attempts = 0usize;
        let mut slept = 0usize;
        let mut init = || -> Result<i32, String> {
            attempts += 1;
            Err("DeviceLost".to_string())
        };

        assert!(init_with_retry_and_sleep(&mut init, |_d| slept += 1).is_err());
        // 3 retries + 1 final attempt
        assert_eq!(attempts, 4);
        assert_eq!(slept, 3);
    }
}
