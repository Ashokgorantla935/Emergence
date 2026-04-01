/// Baked heightmap assets embedded at compile time.
pub mod earth {
    pub const ELEVATION_256: &[u8] =
        include_bytes!("../../../../assets/maps/earth_256.elevation");
    pub const WATER_256: &[u8] =
        include_bytes!("../../../../assets/maps/earth_256.water");
}

pub mod mars {
    pub const ELEVATION_256: &[u8] =
        include_bytes!("../../../../assets/maps/mars_256.elevation");
}
