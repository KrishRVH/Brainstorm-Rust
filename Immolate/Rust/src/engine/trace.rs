use crate::filters::{FilterConfig, apply_filters};
use crate::instance::Instance;
use crate::seed::Seed;

pub fn apply_current_trace(seed: &Seed, cfg: &FilterConfig) -> bool {
    let mut inst = Instance::new(seed.clone());
    apply_filters(&mut inst, cfg)
}
