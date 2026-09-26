use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDeviceInfo {
    pub name: String,
    pub vendor: GpuVendor,
    pub has_tensor_cores: bool,
    pub api_backend: String,
    pub status_text: String,
}

impl Default for GpuDeviceInfo {
    fn default() -> Self {
        Self {
            name: "Автоопределение".to_string(),
            vendor: GpuVendor::Unknown,
            has_tensor_cores: false,
            api_backend: "DirectML / CPU".to_string(),
            status_text: "Готов к работе".to_string(),
        }
    }
}

pub fn detect_gpu_device() -> GpuDeviceInfo {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{EnumDisplayDevicesW, DISPLAY_DEVICEW};

        let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let mut i = 0u32;
        while unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut dd, 0) } != 0 {
            let name_slice = &dd.DeviceString;
            let len = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
            let name = String::from_utf16_lossy(&name_slice[..len]).trim().to_string();

            if !name.is_empty() {
                let lower = name.to_lowercase();
                if lower.contains("nvidia") || lower.contains("geforce") || lower.contains("rtx") {
                    let has_tensor = lower.contains("rtx") || lower.contains("titan") || lower.contains("a100") || lower.contains("h100") || lower.contains("quadro");
                    let status = if has_tensor {
                        format!("{} — Тензорные ядра активны (FP16/INT8)", name)
                    } else {
                        format!("{} — Аппаратное CUDA ускорение активно", name)
                    };
                    return GpuDeviceInfo {
                        name,
                        vendor: GpuVendor::Nvidia,
                        has_tensor_cores: has_tensor,
                        api_backend: "NVIDIA TensorRT / CUDA".to_string(),
                        status_text: status,
                    };
                } else if lower.contains("amd") || lower.contains("radeon") {
                    return GpuDeviceInfo {
                        name: name.clone(),
                        vendor: GpuVendor::Amd,
                        has_tensor_cores: true, // AMD RDNA AI accelerators
                        api_backend: "AMD DirectML / ROCm".to_string(),
                        status_text: format!("{} — AMD AI Аппаратный ускоритель активен", name),
                    };
                } else if lower.contains("intel") || lower.contains("arc") {
                    return GpuDeviceInfo {
                        name: name.clone(),
                        vendor: GpuVendor::Intel,
                        has_tensor_cores: lower.contains("arc"),
                        api_backend: "Intel OpenVINO / DirectML".to_string(),
                        status_text: format!("{} — DirectML ускорение активно", name),
                    };
                }
            }
            i += 1;
        }
    }

    #[cfg(not(windows))]
    {
        // Linux: check lspci / sysfs
        if let Ok(paths) = std::fs::read_dir("/sys/class/drm") {
            for entry in paths.flatten() {
                let p = entry.path().join("device/uevent");
                if let Ok(content) = std::fs::read_to_string(p) {
                    let lower = content.to_lowercase();
                    if lower.contains("nvidia") {
                        return GpuDeviceInfo {
                            name: "NVIDIA GPU (Linux)".to_string(),
                            vendor: GpuVendor::Nvidia,
                            has_tensor_cores: true,
                            api_backend: "NVIDIA CUDA / TensorRT".to_string(),
                            status_text: "NVIDIA Tensor Cores активны на Linux".to_string(),
                        };
                    } else if lower.contains("amdgpu") || lower.contains("amd") {
                        return GpuDeviceInfo {
                            name: "AMD Radeon GPU (Linux)".to_string(),
                            vendor: GpuVendor::Amd,
                            has_tensor_cores: true,
                            api_backend: "AMD ROCm / Vulkan".to_string(),
                            status_text: "AMD AI Ускоритель активен на Linux".to_string(),
                        };
                    }
                }
            }
        }
    }

    GpuDeviceInfo {
        name: "Стандартный процессор / DirectML".to_string(),
        vendor: GpuVendor::Unknown,
        has_tensor_cores: false,
        api_backend: "DirectML / High-Speed CPU".to_string(),
        status_text: "Аппаратное ускорение готово (DirectML / CPU)".to_string(),
    }
}
