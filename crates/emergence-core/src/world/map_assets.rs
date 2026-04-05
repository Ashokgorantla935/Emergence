/// Baked heightmap assets embedded at compile time.
pub mod earth {
    pub const ELEVATION_256: &[u8] =
        include_bytes!("../../../../assets/maps/earth_256.elevation");
    pub const WATER_256: &[u8] =
        include_bytes!("../../../../assets/maps/earth_256.water");
    pub const ELEVATION_4096: &[u8] =
        include_bytes!("../../../../assets/maps/earth_4096.elevation");
    pub const WATER_MASK_4096: &[u8] =
        include_bytes!("../../../../assets/maps/earth_4096.water");
}

pub mod mars {
    pub const ELEVATION_256: &[u8] =
        include_bytes!("../../../../assets/maps/mars_256.elevation");
}
