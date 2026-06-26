use crate::hardware::types::{GpuInfo, GpuVendor};

#[allow(dead_code)]
pub fn detect_amd() -> Vec<GpuInfo> {
    // ponytail: ROCm detection via sysfs. Stub for now, expand for Linux AMD.
    vec![]
}
