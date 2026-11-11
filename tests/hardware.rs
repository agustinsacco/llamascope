//! Local sensor parsers. Probes are optional; bad readings become unavailable, never zero.
use llamascope::hardware::{parse_meminfo, parse_nvidia_smi, parse_rocm, parse_vm_stat};

/// `rocm-smi --showuse --showtemp --showpower --showmeminfo vram gtt --json` on a Strix Halo APU.
const ROCM_APU: &str = r#"{"card0": {"Temperature (Sensor edge) (C)": "97.0", "Current Socket Graphics Package Power (W)": "84.093", "GPU use (%)": "90", "VRAM Total Memory (B)": "2147483648", "VRAM Total Used Memory (B)": "2144800768", "GTT Total Memory (B)": "137438953472", "GTT Total Used Memory (B)": "67516796928"}}"#;

#[test]
fn rocm_apu_reports_vram_and_gtt() {
    let gpu = parse_rocm(ROCM_APU).unwrap();
    assert_eq!(gpu.usage, Some(90.0));
    assert_eq!(gpu.temp, Some(97.0));
    assert_eq!(gpu.power, Some(84.093));
    assert_eq!(gpu.vram.unwrap().total_gib, 2.0);
    let gtt = gpu.gtt.unwrap();
    assert_eq!(gtt.total_gib, 128.0);
    assert!((gtt.percent() - 49.1).abs() < 0.1);
    assert!(gpu.source.contains("rocm-smi"));
}
#[test]
fn rocm_invalid_values_are_unavailable_not_zero() {
    let gpu = parse_rocm(r#"{"card0":{"GPU use (%)":"45.2","Temperature (Sensor edge) (C)":120.5,"Current Socket Graphics Package Power (W)":"invalid"}}"#).unwrap();
    assert_eq!(gpu.usage, Some(45.2));
    assert_eq!(gpu.temp, Some(120.5));
    assert_eq!(gpu.power, None, "unavailable readings are not zero watts");
    assert!(parse_rocm(r#"{"card0":{"GPU use (%)":"NaN"}}"#).is_none());
    assert!(parse_rocm(r#"{"card1":{"GPU use (%)":"5"}}"#).is_none());
}
#[test]
fn rocm_values_are_clamped_and_memory_is_gib() {
    let gpu = parse_rocm(r#"{"card0":{"GPU use (%)":"120","VRAM Total Memory (B)":"2147483648","VRAM Total Used Memory (B)":"1073741824"}}"#).unwrap();
    assert_eq!(gpu.usage, Some(100.0));
    let memory = gpu.vram.unwrap();
    assert_eq!(memory.used_gib, 1.0);
    assert_eq!(memory.percent(), 50.0);
    assert!(gpu.gtt.is_none());
}
#[test]
fn nvidia_smi_csv_and_unsupported_fields() {
    let gpu = parse_nvidia_smi("87, 71, 245.31, 12288, 24576\n1, 30, 20, 0, 24576\n").unwrap();
    assert_eq!(gpu.usage, Some(87.0));
    assert_eq!(gpu.temp, Some(71.0));
    assert_eq!(gpu.power, Some(245.31));
    assert_eq!(gpu.vram.unwrap().total_gib, 24.0);
    assert!(gpu.source.contains("nvidia-smi"));
    let partial = parse_nvidia_smi("12, 40, [N/A], 100, 1000").unwrap();
    assert_eq!(partial.power, None);
    for bad in ["", "1, 2, 3", "[N/A], [N/A], [N/A], [N/A], [N/A]"] {
        assert!(parse_nvidia_smi(bad).is_none(), "accepted {bad:?}");
    }
}
#[test]
fn linux_memory_does_not_depend_on_a_gpu() {
    let memory = parse_meminfo("MemTotal: 8388608 kB\nMemAvailable: 2097152 kB\n").unwrap();
    assert_eq!(memory.total_gib, 8.0);
    assert_eq!(memory.used_gib, 6.0);
    assert!(parse_meminfo("MemTotal: 1 kB\nMemAvailable: 2 kB").is_none());
    assert!(parse_meminfo("MemTotal: NaN kB\nMemAvailable: 0 kB").is_none());
}
#[test]
fn macos_memory_uses_the_reported_page_size() {
    let memory = parse_vm_stat("1073741824", "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free: 16384.\nPages inactive: 16384.\nPages speculative: 0.\n").unwrap();
    assert_eq!(memory.total_gib, 1.0);
    assert_eq!(memory.used_gib, 0.5);
}
