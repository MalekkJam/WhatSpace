#[allow(unknown_lints)]
#[allow(clippy::all)]
#[allow(unused_attributes)]
#[allow(dead_code)]
#[allow(missing_docs)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
#[allow(non_upper_case_globals)]
#[allow(trivial_casts)]
#[allow(unused_results)]
#[allow(unused_mut)]
pub mod inner {
    include!(concat!(env!("OUT_DIR"), "/proto/bundle.rs"));
}

pub use inner::BundleKind;
pub use inner::MsgStatus;
pub use inner::ProtobufBundle;
