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
pub trait BytesWR<W : Write, R : Read> {

  /// used to know if we are in reading or not (header required)
  /// only used for default implementation of read without start
  fn has_started(&self) -> bool;

  /// possibly read some header
  fn start_read(&mut self, &mut R) -> Result<()>;

  /// possibly write some header
  fn start_write(&mut self, &mut W) -> Result<()>;


  /// return 0 if ended (content might still be read afterward on reader but endof BytesWR
  fn b_read(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_read(r));
    }
    r.read(buf)
  }

  fn b_write(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_write(w));
    }
    w.write(cont)
  }

  /// end read : we know the read is complete (for instance rust serialize object decoded), some
  /// finalize operation may be added (for instance read/drop padding bytes).
  fn end_read(&mut self, &mut R) -> Result<()>;

  /// end of content write
  fn end_write(&mut self, &mut W) -> Result<()>;
}


pub struct BWRImplW<'a, W : 'a + Write, BWR : 'a + BytesWR<W,VoidRead>> (&'a mut BWR, VoidRead, &'a mut W);
pub struct BWRImplR<'a, R : 'a + Read, BWR : 'a + BytesWR<VoidWrite,R>> (&'a mut BWR, &'a mut R, VoidWrite);

impl<'a, W : Write, BWR : BytesWR<W,VoidRead>> BWRImplW<'a, W, BWR> {
  pub fn new(bwr : &'a mut BWR, w : &'a mut W) -> Self {
    BWRImplW(bwr, VoidRead, w)
  }
}
impl<'a, R : Read, BWR : BytesWR<VoidWrite,R>> BWRImplR<'a, R, BWR> {
  pub fn new(bwr : &'a mut BWR, r : &'a mut R) -> Self {
    BWRImplR(bwr, r, VoidWrite)
  }
}

/// automatic end of of bmr read or write impl, include only if type will not fail on end
pub mod unsafe_bwrimpl_autoend {
  use std::io::{
    Write,
    Read,
    Result,
  };
  use std::ops::Drop;
  use super::{
    BWRImplW,
    BWRImplR,
    BytesWR,
    VoidRead,
    VoidWrite,
  };


  impl<'a, W : Write, BWR : BytesWR<W,VoidRead>> Drop for BWRImplW<'a,W,BWR> {
   fn drop(&mut self) {
     self.0.end_write(&mut self.2).unwrap();
   }
  }
  impl<'a, R : Read, BWR : BytesWR<VoidWrite,R>> Drop for BWRImplR<'a,R,BWR> {
   fn drop(&mut self) {
     self.0.end_read(&mut self.1).unwrap();
   }
  }
}


impl<'a,R : Read, BWR : BytesWR<VoidWrite,R>> Read for BWRImplR<'a,R,BWR> {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    self.0.b_read(&mut self.1, buf)
  }
}
impl<'a,W : Write, BWR : BytesWR<W,VoidRead>> Write for BWRImplW<'a,W,BWR> {
  fn write(&mut self, cont: &[u8]) -> Result<usize> {
    self.0.b_write(&mut self.2, cont)
  }
  /// do not end_write (for flexibility and symetry with read impl)
  fn flush(&mut self) -> Result<()> {
    self.2.flush()
  }
}

pub struct VoidRead;
pub struct VoidWrite;
impl Read for VoidRead {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
    Ok(0)
  }
}
impl Write for VoidWrite {
  fn write(&mut self, cont: &[u8]) -> Result<usize> {
    Ok(0)
  }
  fn flush(&mut self) -> Result<()> {
    Ok(())
  }
}


/// an adaptable (or fix) content is send, at the end if bytes 0 end otherwhise skip byte and read
/// another window
pub mod sized_windows {

use std::io::{
  Write,
  Read,
  Result,
  Error as IoError,
  ErrorKind as IoErrorKind,
};
use bytes_wr::BytesWR;
use std::marker::PhantomData;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

  /// conf trait
  pub trait SizedWindowsParams {
    const INIT_SIZE : usize;
    const GROWTH_RATIO : Option<(usize,usize)>;
    /// size of window is written, this way INIT_size and growth_ratio may diverge (multiple
    /// profiles)
    const WRITE_SIZE : bool;
  }

  pub struct SizedWindows<W : Write, R : Read, P : SizedWindowsParams>  {
    init_size : Option<usize>,
    winrem : usize,
    _wr : PhantomData<(W,R,P)>,
  }

  impl<W : Write, R : Read, P : SizedWindowsParams>  SizedWindows<W, R, P> {
    /// p param is only to simplify type inference (it is an empty struct)
    pub fn new (_ : P) -> Self {
      SizedWindows {
        init_size : None, // contain current win size or None if not started, Some(0) if ended
        winrem : P::INIT_SIZE,
        _wr : PhantomData,
      }
    }
  }

impl<W : Write, R : Read, P : SizedWindowsParams> BytesWR<W, R> for SizedWindows<W, R, P> {
  #[inline]
  fn has_started(&self) -> bool {
    self.init_size.is_some()
  }

  #[inline]
  fn start_write(&mut self, w : &mut W) -> Result<()> {
    if P::WRITE_SIZE {
      try!(w.write_u64::<LittleEndian>(self.winrem as u64));
    }
    self.init_size = Some(self.winrem);
    Ok(())
  }


  #[inline]
  fn start_read(&mut self, r : &mut R) -> Result<()> {
    if P::WRITE_SIZE {
      self.winrem = try!(r.read_u64::<LittleEndian>()) as usize;
    }
    self.init_size = Some(self.winrem);
    Ok(())
  }

  fn b_read(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_read(r));
    }
    if let Some(0) = self.init_size {
      // ended read (still padded)
      return Ok(0);
    }
    if self.winrem == 0 {
      let mut b = [0];
      let rr = try!(r.read(&mut b));
      if rr != 1 {
        return
         Err(IoError::new(IoErrorKind::Other, "No bytes after window size, do not know if ended or repeat"));
      }
      if b[0] == 0 {
        // ended (case where there is no padding or we do not know what we read and read also
        // the padding)
        self.init_size = Some(0);
        return Ok(0)
      } else {
        // new window and drop this byte
        self.winrem = if P::WRITE_SIZE {
          try!(r.read_u64::<LittleEndian>()) as usize
        } else {
          match P::GROWTH_RATIO {
            Some((n,d)) => self.init_size.unwrap() * n / d,
            None => P::INIT_SIZE,
          }
        };
        self.init_size = Some(self.winrem);
      }
    }
    let rr = if self.winrem < buf.len() {
      try!(r.read(&mut buf[..self.winrem]))
    } else {
      try!(r.read(buf))
    };
    self.winrem -= rr;
    Ok(rr)
  }

  fn b_write(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_write(w));
    }
    let mut tot = 0;
    while tot < cont.len() {
      if self.winrem + tot < cont.len() {
        let ww = try!(w.write(&cont[tot..tot + self.winrem]));
        tot += ww;
        self.winrem -= ww;
        if self.winrem == 0 {
          // init next winrem
          self.winrem = match P::GROWTH_RATIO {
              Some((n,d)) => self.init_size.unwrap() * n / d,
              None => P::INIT_SIZE,
          };
          self.init_size = Some(self.winrem);
        }
        // non 0 (terminal) value
        try!(w.write(&[1]));
        if P::WRITE_SIZE {
          try!(w.write_u64::<LittleEndian>(self.winrem as u64));
        };
      } else {
        let ww = try!(w.write(&cont[tot..]));
        tot += ww;
        self.winrem -= ww;
      }
    }
    Ok(tot)
  }

  /// read padding and error if next bit is not end
  fn end_read(&mut self, r : &mut R) -> Result<()> {
    // TODO buffer is needed here -> see if Read interface should not have a fn drop where we read
    // without buffer and drop content. For now hardcoded buffer length...
    let mut buffer = [0; 256];

    if let Some(0) = self.init_size {
      self.init_size = None;
      self.winrem = P::INIT_SIZE;
      return Ok(());
    }

    while self.winrem != 0 {
      let ww = if self.winrem > 256 {
        try!(r.read(&mut buffer))
      } else {
        try!(r.read(&mut buffer[..self.winrem]))
      };
      self.winrem -= ww;
    }
    let ww = try!(r.read(&mut buffer[..1]));
    if ww != 0 || buffer[0] != 0 {
      error!("\n{}\n",buffer[0]);
       return
         Err(IoError::new(IoErrorKind::Other, "End read does not find expected terminal 0 of widows"));
    }
    // init as new
    self.init_size = None;
    self.winrem = P::INIT_SIZE;
 
    Ok(())
  }

  /// end of content write
  fn end_write(&mut self, r : &mut W) -> Result<()> {
    // TODO buffer is not nice here
    let mut buffer = [0; 256];

    while self.winrem != 0 {
      let ww = if self.winrem > 256 {
        try!(r.write(&mut buffer))
      } else {
        try!(r.write(&mut buffer[..self.winrem]))
      };
      self.winrem -= ww;
    }
    // terminal 0
    try!(r.write(&[0]));
    // init as new
    self.init_size = None;
    self.winrem = P::INIT_SIZE;
    Ok(())

  }
}



}


/// an end byte is added at the end
/// This sequence is escaped in stream through escape bytes
/// Mostly for test purpose (read byte per byte).
pub mod escape_term {
  // TODO if esc char check for esc seq
  // if esc char in content wr esc two times
use std::io::{
  Write,
  Read,
  Result,
};
use bytes_wr::BytesWR;
use std::marker::PhantomData;


  /// contain esc byte, and if started, and if escaped, and if waiting for end read
  pub struct EscapeTerm<W : Write, R : Read> (u8,bool,bool,bool,PhantomData<(W,R)>);
  
  impl<W : Write, R : Read> EscapeTerm<W, R> {
    pub fn new (t : u8) -> Self {
      EscapeTerm (t, false,false,false,PhantomData)
    }
  }

impl<W : Write, R : Read> BytesWR<W, R> for EscapeTerm<W, R> {

  #[inline]
  fn has_started(&self) -> bool {
    self.1
  }

  #[inline]
  fn start_read(&mut self, _ : &mut R) -> Result<()> {
    self.1 = true;
    Ok(())
  }

  #[inline]
  fn start_write(&mut self, _ : &mut W) -> Result<()> {
    self.1 = true;
    Ok(())
  }


  /// return 0 if ended (content might still be read afterward on reader but endof BytesWR
  fn b_read(&mut self, r : &mut R, buf: &mut [u8]) -> Result<usize> {
    let mut b = [0];
    if self.3 {
      return Ok(0);
    }
    let hs = self.has_started();
    if !self.has_started() {
      try!(self.start_read(r));
    }
    let mut i = 0;
    while i < buf.len() {
      let rr = try!(r.read(&mut b[..]));
      if rr == 0 {
        return Ok(i);
      }
      if b[0] == self.0 {
        if self.2 {
        debug!("An escaped char read effectively");
          buf[i] = b[0];
          i += 1;
          self.2 = false;
        } else {
        debug!("An escaped char read start");
          self.2 = true;
        }
      } else {
        if self.2 {
        debug!("An escaped end");
          // end
          self.3 = true;
          return Ok(i);
        } else {
          buf[i] = b[0]; 
          i += 1;
        }
      }

    }
    Ok(i)
  }

  fn b_write(&mut self, w : &mut W, cont: &[u8]) -> Result<usize> {
    if !self.has_started() {
      try!(self.start_write(w));
    }
    for i in 0.. cont.len() {
      let b = cont[i];
      if b == self.0 {
        debug!("An escaped char");
        try!(w.write(&cont[i..i+1]));
        try!(w.write(&cont[i..i+1]));
      } else {
        try!(w.write(&cont[i..i+1]));
      }
    }
    Ok(cont.len())
  }

  /// end read : we know the read is complete (for instance rust serialize object decoded), some
  /// finalize operation may be added (for instance read/drop padding bytes).
  fn end_read(&mut self, _ : &mut R) -> Result<()> {
    self.1 = false;
    self.3 = false;
    Ok(())
  }

  /// end of content write
  fn end_write(&mut self, w : &mut W) -> Result<()> {
    self.1 = false;
    let two = if self.0 == 0 {
      try!(w.write(&[self.0,1]))
    }else {
      try!(w.write(&[self.0,0]))
    };
    // TODO clean io error if two is not 2
    assert!(two == 2);
    Ok(())

  }
}


}


