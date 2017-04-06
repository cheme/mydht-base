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
  TunnelWriterEW,
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
use super::MultipleReplyMode;
use super::TunnelCacheManager;

pub struct Full {
  pub reply_mode : MultipleReplyMode,
  pub error_mode : MultipleReplyMode,
}
/**
 * No impl for instance when no error or no reply
 */
pub struct FullW<RI : Info, EI : Info> {
  pub state: TunnelState,
  pub reply_info : RI,
  pub error_info : EI,
  pub current_cache_id: Option<Vec<u8>>,
}

pub struct FullR {
}

pub struct FullSRW {
}

// TODO move fn to TunnelManager
impl  Full {

  pub fn new_rep<RI : Info, EI : Info> (&self, ri : RI, ei : EI) -> FullW<RI,EI> {
    FullW {
      current_cache_id : None,
      state : TunnelState::QueryOnce,
      reply_info : ri,
      error_info : ei,
    }
  }


  pub fn new_no_rep<RI : Info, EI : Info> (&self, ri : RI, ei : EI) -> FullW<RI,EI> {
    FullW {
      current_cache_id : None,
      state : TunnelState::ReplyOnce,
      reply_info : ri,
      error_info : ei,
    }
  }

  pub fn new_with_sym<RI : Info, EI : Info, TC : TunnelCacheManager<FullSRW>> (&self, ri : RI, ei : EI, tc : &mut TC) -> FullW<RI,EI> {
    let comid = if ri.do_cache() || ei.do_cache() {
      let fsrw = FullSRW {    };
      Some(tc.storeW(fsrw))
    } else {
      None
    };
    FullW{
      current_cache_id : comid,
      state : TunnelState::QueryCached,
      reply_info : ri,
      error_info : ei,
    }
  }
}


/// TODO this impl must move to all TunnelWriter
impl<E : ExtWrite, P : Peer, RI : Info, EI : Info, TW : TunnelWriter<E, P, RI, EI>> ExtWrite for TunnelWriterEW<E,P,RI,EI,TW> {
  #[inline]
  fn write_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(self.0.write_state(w));
    try!(self.0.write_connect_info(w));

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


impl<E : ExtWrite, P : Peer, RI : Info, EI : Info> TunnelWriter<E, P, RI, EI> for FullW<RI,EI> {
  #[inline]
  fn write_state<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.state, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_error_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.error_info, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_reply_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    try!(bin_encode(&self.reply_info, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  #[inline]
  fn write_connect_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    if let Some(cci) = self.current_cache_id.as_ref() {
      try!(bin_encode(cci, w, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    }
    Ok(())
  }


}

// impl<E : ExtWrite, P : Peer, RI : Info> TunnelReplyWriter<E, P, RI> for FullSRW {}

impl<E : ExtWrite, P : Peer, RI : Info, EI : Info> TunnelErrorWriter<E, P, EI> for FullW<RI,EI> {
}

