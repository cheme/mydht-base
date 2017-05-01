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
  TunnelCache,
  TunnelErrorWriter,
  TunnelReader,
  Info,
  RepInfo,
  SymProvider,
  RouteProvider,
  ErrorProvider,
  ReplyProvider,
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
pub struct Nope;


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
}
impl RepInfo for Nope {
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
  fn write_tunnel_header<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}
  #[inline]
  fn write_simkeys_into< W : Write>( &mut self, _ : &mut W) -> Result<()> {
    unimplemented!()
  }
  #[inline]
  fn write_tunnel_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<usize> {Ok(cont.len())}
  #[inline]
  fn write_tunnel_all_into<W : Write>(&mut self, w : &mut W, cont : &[u8]) -> Result<()> {Ok(())}
  #[inline]
  fn flush_tunnel_into<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}
  #[inline]
  fn write_tunnel_end<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}

}

impl TunnelErrorWriter for Nope {
  fn write_error<W : Write>(&mut self, _ : &mut W, _ : usize) -> Result<()> {
    Ok(())
  }
}
impl TunnelReader for Nope {
}

impl<SSW,SSR> TunnelCache<SSW,SSR> for Nope {
  fn put_symw_tunnel(&mut self, _ : SSW, _ : Vec<u8>) -> Result<()> {
    // TODO replace with actual erro
    unimplemented!()
  }
  fn get_symw_tunnel(&mut self, _ : &[u8]) -> Result<&mut SSW> {
    // TODO replace with actual erro
    unimplemented!()
  }
  fn has_symw_tunnel(&mut self, _ : &[u8]) -> bool {
    false
  }

  fn put_symr_tunnel(&mut self, _ : SSR) -> Result<Vec<u8>> {
    // TODO replace with actual erro
    unimplemented!()
  }
  fn get_symr_tunnel(&mut self, _ : &[u8]) -> Result<&mut SSR> {
    // TODO replace with actual erro
    unimplemented!()
  }
  fn has_symr_tunnel(&mut self, _ : &[u8]) -> bool {
    false
  }
  fn new_cache_id (&mut self) -> Vec<u8> {
    vec![]
  }
}
// TODO remove as only for dev progress
impl<SSW,SSR,P> SymProvider<SSW,SSR,P> for Nope {

  fn new_sym_key (&mut self, _ : &P) -> Vec<u8> {
    panic!("Should only be use for dev");
  }
  fn new_sym_writer (&mut self, _ : Vec<u8>) -> SSW {
    panic!("Should only be use for dev");
  }
  fn new_sym_reader (&mut self, _ : Vec<u8>) -> SSR {
    panic!("Should only be use for dev");
  }

}

impl<P : Peer> RouteProvider<P> for Nope {

  fn new_route (&mut self, _ : &P) -> Vec<&P> {
    panic!("Placeholder, should not be called");
  }
  fn new_reply_route (&mut self, _ : &P) -> Vec<&P> {
    panic!("Placeholder, should not be called");
  }

}

impl<P : Peer, EI : Info> ErrorProvider<P,EI> for Nope {
  fn new_error_route (&mut self, _ : &[&P]) -> Vec<EI> {
    panic!("Placeholder, should not be called");
  }
}
impl<P : Peer, RI : RepInfo,SSW,SSR> ReplyProvider<P,RI,SSW,SSR> for Nope {
  fn new_reply (&mut self, _ : &[&P]) -> Vec<RI> {
    panic!("Placeholder, should not be called");
  }
}


