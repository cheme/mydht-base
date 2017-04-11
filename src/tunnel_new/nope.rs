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
  TunnelReader,
  Info,
};
use std::io::{
  Write,
  Read,
  Result,
};
use peer::Peer;


/**
 * No impl for instance when no error or no reply
 */
pub struct Nope ();


impl Info for Nope {
  #[inline]
  fn do_cache (&self) -> bool {
    false
  }
  #[inline]
  fn write_in_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    Ok(())
  }
  #[inline]
  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()> {
    Ok(())
  }
  #[inline]
  fn get_reply_key(&self) -> Option<&Vec<u8>> {
    None
  }

}
impl ExtWrite for Nope {
  #[inline]
  fn write_header<W : Write>(&mut self, _ : &mut W) -> Result<()> {
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

impl ExtRead for Nope {
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


impl TunnelWriter for Nope {
  #[inline]
  fn write_state<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}
  #[inline]
  fn write_connect_info<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}
  #[inline]
  fn write_simkeys_into< W : Write>( &mut self, _ : &mut W) -> Result<()> {
    unimplemented!()
  }

}

impl TunnelReader for Nope {
}

