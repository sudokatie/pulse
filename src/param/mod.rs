//! Parameter system

mod automation;
mod types;
mod value;
mod smooth;

pub use automation::{AutomationCurve, AutomationLane, AutomationManager, AutomationPoint};
pub use types::{ParamInfo, ParamType};
pub use value::ParamValue;
pub use smooth::ParamSmoother;
