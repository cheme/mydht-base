use readwrite_comp::{
  MultiW,
  MultiWExt,
  MultiRExt,
  ExtRead,
  ExtWrite,
  CompW,
  CompWState,
  CompR,
  CompRState,
  CompExtW,
  CompExtWInner,
  CompExtR,
  CompExtRInner,
};
use super::{
  TunnelWriter,
  TunnelState,
  TunnelErrorWriter,
  TunnelReplyWriter,
  Info,
};
use std::io::{
  Write,
  Read,
  Result,
};
use peer::Peer;
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use super::BincErr;

pub struct Full {
}
/**
 * No impl for instance when no error or no reply
 */
pub struct FullW {
  pub state: TunnelState,
}

pub struct FullR {
}

pub struct FullSRW {
}

impl FullW {
  pub fn new_rep () -> FullW {
    FullW {
      state : TunnelState::QueryOnce,
    }
  }


  pub fn new_no_rep () -> FullW {
    FullW {
      state : TunnelState::ReplyOnce,
    }
  }

  pub fn new_with_sym () -> (FullW, FullSRW) {
    (FullW{
      state : TunnelState::QueryCached,
    },
    FullSRW {
    })
  }
}


impl ExtWrite for FullW {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));

    Ok(())
  }

  #[inline]
  fn write_into<W : Write>(&mut self, _ : &mut W, _ : &[u8]) -> Result<usize> {
    Ok(0)
  }

  #[inline]
  fn write_all_into<W : Write>(&mut self, _ : &mut W, _ : &[u8]) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn flush_into<W : Write>(&mut self, _ : &mut W) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn write_end<W : Write>(&mut self, _ : &mut W) -> Result<()> {
    Ok(())
  }
}

impl ExtRead for Full {
  #[inline]
  fn read_header<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn read_from<R : Read>(&mut self, _ : &mut R, _ : &mut[u8]) -> Result<usize> {
    Ok(0)
  }

  #[inline]
  fn read_exact_from<R : Read>(&mut self, _ : &mut R, _ : &mut[u8]) -> Result<()> {
    Ok(())
  }

  #[inline]
  fn read_end<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }
}


impl<E : ExtWrite, P : Peer, RI : Info, EI : Info> TunnelWriter<E, P, RI, EI> for FullW {
}

// impl<E : ExtWrite, P : Peer, RI : Info> TunnelReplyWriter<E, P, RI> for FullSRW {}

impl<E : ExtWrite, P : Peer, EI : Info> TunnelErrorWriter<E, P, EI> for FullW {
}

