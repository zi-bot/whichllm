#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
    pub bandwidth_gbps: f64,
    pub vendor: GpuVendor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Apple,
    #[allow(dead_code)]
    Intel,
    #[allow(dead_code)]
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub gpus: Vec<GpuInfo>,
    pub cpu: CpuInfo,
    pub ram_gb: f64,
    pub os: OsType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsType {
    Linux,
    MacOS,
    Windows,
    Unknown,
}
