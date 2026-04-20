pub const DESKTOP_API_HOST: &str = "127.0.0.1";
pub const DESKTOP_API_PORT: u16 = 3000;
pub const DESKTOP_API_BIND: &str = "127.0.0.1:3000";
pub const DESKTOP_API_ORIGIN: &str = "http://127.0.0.1:3000";

pub const DESKTOP_DEV_ALLOWED_ORIGINS: [&str; 4] = [
    "http://localhost:1420",
    "http://127.0.0.1:1420",
    "http://cn.cyberhan.slab.localhost",
    "cn.cyberhan.slab://localhost",
];

pub const fn desktop_api_host() -> &'static str {
    DESKTOP_API_HOST
}

pub const fn desktop_api_port() -> u16 {
    DESKTOP_API_PORT
}

pub const fn desktop_api_bind() -> &'static str {
    DESKTOP_API_BIND
}

pub const fn desktop_api_origin() -> &'static str {
    DESKTOP_API_ORIGIN
}

pub const fn desktop_dev_allowed_origins() -> &'static [&'static str; 4] {
    &DESKTOP_DEV_ALLOWED_ORIGINS
}
