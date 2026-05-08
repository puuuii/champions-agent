pub struct DamageInput {
    pub attacker_id: u32,
    pub defender_id: u32,
    pub move_id: u32,
    pub attacker_ap: [u32; 6],
    pub defender_ap: [u32; 6],
    pub attacker_nature_id: u32,
    pub defender_nature_id: u32,
    pub attacker_stages: [i8; 8],
    pub defender_stages: [i8; 8],
    pub is_critical: bool,
    pub rng_roll: f64,
}
