use alloc::string::ToString;
use alloc::vec::Vec;
use crate::identity::CanonicalSemanticBasis;

pub trait Source {
    fn id(&self) -> &str;
}

pub struct Observation<S: Source> {
    pub source: S,
    pub timestamp: u64,
    pub payload: Vec<u8>,
}

impl<S: Source> CanonicalSemanticBasis for Observation<S> {
    fn canonical_basis(&self) -> &[u8] {
        let s = format!("{:?}|{:?}|{:?}", self.source.id(), self.timestamp, self.payload);
        let boxed = s.into_boxed_str();
        let leaked: &'static mut str = Box::leak(boxed);
        leaked.as_bytes()
    }
}

impl<S: Source> Observation<S> {
    pub fn new(source: S, timestamp: u64, payload: Vec<u8>) -> Result<Self, String> {
        if source.id().is_empty() {
            return Err("Source ID cannot be empty".to_string());
        }
        Ok(Observation { source, timestamp, payload })
    }
}