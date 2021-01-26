use std::mem::size_of;
use std::ptr::null_mut;
use std::iter::IntoIterator;
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct TracedInstruction {
    pub haddr: u64,
    pub vaddr: u64,
    pub size: u8,
    pub data: [u8;16],
}
pub struct Trace {
    buffer: Vec<u8>,
}
pub struct TraceIter<T> {
    buffer: Vec<u8>,
    iter_location: *const T,
    iter_bytes_covered: usize,
}

impl Trace {
    pub fn new(buffer: Vec<u8>) -> Self {
       Trace { buffer }
    }
}

impl From<Vec<u8>> for Trace {
    fn from(buffer: Vec<u8>) -> Self {
        Trace::new(buffer)
    }
}

impl IntoIterator for Trace {
    type Item = TracedInstruction;
    type IntoIter = TraceIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
         TraceIter { buffer: self.buffer, iter_location: null_mut(), iter_bytes_covered: 0 }
    }
}

impl Iterator for TraceIter<TracedInstruction> {
    type Item = TracedInstruction;

    fn next(&mut self) -> Option<Self::Item> {
        let bytes_covered = self.iter_bytes_covered;
        if bytes_covered == 0 {
            self.iter_location = self.buffer.as_ptr().cast::<TracedInstruction>();
        }
        if bytes_covered >= self.buffer.len() {
            None
        } else {
            unsafe {
                let item = self.iter_location;
                self.iter_location = self.iter_location.offset(1);
                self.iter_bytes_covered += size_of::<TracedInstruction>();
                Some(*item)
            }
        }
    }
}