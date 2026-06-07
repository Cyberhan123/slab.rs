pub(super) const fn flash_attn_enabled() -> bool {
    true
}

pub(super) const fn auto_unload_min_free_system_memory_bytes() -> u64 {
    1_073_741_824
}

pub(super) const fn auto_unload_min_free_gpu_memory_bytes() -> u64 {
    536_870_912
}

pub(super) const fn auto_unload_max_pressure_evictions_per_load() -> u32 {
    3
}
