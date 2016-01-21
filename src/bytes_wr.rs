//! bytes_wr : reader and writer (adapter) when no knowledge of content length (ie a byte stream)
//! The point is to be able to write content without knowledge of the final lenghth and without
//! loading all bytes in memory (only buffer). 
//! Common usecase is tunnel proxy where we do not know the length of content written (encyphered
//! bytes or encodable unknow content), the point is to avoid mismatch between this content and
//! further content.
//!

use std::io::{
  Write,
  Read,
  Result,
};

/// note should only be use for read or write at the same time :
/// should be split in two traits
trait BytesWR<W : Write, R : Read> : Read + Write {

  /// used to know if we are in reading or not (header required)
  /// only used for default implementation of read without start
  fn has_started(&self) -> bool;

  /// only used for default implementation of read without start
  fn started(&mut self) -> ();

  /// possibly read some header
  fn start_read(&mut self, &mut R) -> Result<()>;

  /// possibly write some header
  fn start_write(&mut self, &mut W) -> Result<()>;


  fn b_read(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_read(r));
      self.started();
    }
    r.read(buf)
  }

  fn b_write(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_write(w));
      self.started();
    }
    w.write(cont)
  }

  /// end read : we know the read is complete (for instance rust serialize object decoded), some
  /// finalize operation may be added (for instance read/drop padding bytes).
  fn end_read(&mut self, &mut R) -> Result<()>;

  /// end of content write
  fn end_write(&mut self, &mut W) -> Result<()>;
}


/// an adaptable (or fix) content is send, at the end if bytes 0 end otherwhise skip byte and read
/// another window
mod sized_windows {
  /// conf trait
  trait SizedWindowsParams {
    const INIT_SIZE : usize;
    const GROWTH_RATIO : Option<usize>;

  }

  struct SizedWindows {
    params : &'static SizedWindowsParams,
  }

  impl SizedWindows {
    pub fn new ( p : &'static SizedWindowsParams) -> Self {
      SizedWindows {
        params : p,
      }
    }
  }
}


/// an end byte is added at the end
/// This sequence is escaped in stream through escape bytes
mod escaped_term {
  // TODO if esc char check for esc seq
  // if esc char in content wr esc two times
}
