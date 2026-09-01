// ECU detector modules, organized by ECU manufacturer.
// Each manufacturer folder groups the detection logic of its ECU families:
//   bosch/ -> EDC15P, EDC15VM, EDC16U1, EDC16U31, EDC16U34 (VAG platforms)
//
// Community contributions: add a new manufacturer folder here (e.g. siemens/,
// delphi/), declare it below, then wire it into smart_detector.rs and
// ecu_identifier.rs. See CONTRIBUTING.md for the full walkthrough.

pub mod bosch;
