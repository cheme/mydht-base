use std::marker::PhantomData;
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
  TunnelNoRep,
  TunnelWriterExt,
  TunnelReaderExt,
  TunnelCache,
  TunnelErrorWriter,
  TunnelReader,
  TunnelReaderNoRep,
  TunnelReaderError,
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
#[derive(Clone)]
pub struct Nope;


impl Info for Nope {
  #[inline]
  fn write_in_header<W : Write>(&mut self, w : &mut W) -> Result<()> {
    Ok(())
  }
  #[inline]
  fn write_read_info<W : Write>(&mut self, w : &mut W) -> Result<()> {
    Ok(())
  }
  #[inline]
  fn read_from_header<R : Read>(r : &mut R) -> Result<Self> {
    Ok(Nope)
  }
  #[inline]
  fn read_read_info<R : Read>(&mut self, r : &mut R) -> Result<()> {
    Ok(())
  }

}
impl RepInfo for Nope {

  #[inline]
  fn require_additional_payload(&self) -> bool {
    false
  }
  #[inline]
  fn do_cache (&self) -> bool {
    false
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
  fn write_tunnel_header<W : Write>(&mut self, _ : &mut W) -> Result<()> {Ok(())}
  #[inline]
  fn write_dest_info< W : Write>( &mut self, _ : &mut W) -> Result<()> {
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

impl TunnelWriterExt for Nope {
  type TW = Nope;
  fn get_writer(&mut self) -> &mut Self::TW {
    self
  }
}
impl TunnelErrorWriter for Nope {
  fn write_error<W : Write>(&mut self, _ : &mut W, _ : usize) -> Result<()> {
    Ok(())
  }
}
impl TunnelReaderNoRep for Nope {
  fn read_state<R : Read> (&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }
  fn read_connect_info<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }
  fn read_tunnel_header<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }

  fn read_dest_info<R : Read>(&mut self, _ : &mut R) -> Result<()> {
    Ok(())
  }
 
}

impl<SSW,SSR> TunnelCache<SSW,SSR> for Nope {
  fn put_symw_tunnel(&mut self, _ : &[u8], _ : SSW) -> Result<()> {
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
impl TunnelReaderExt for Nope {
  type TR = Nope; 
  /// retrieve original inner writer
  fn get_reader(self) -> Self::TR {
    self
  }
}
pub struct TunnelNope<P : Peer> (PhantomData<(P)>);
impl<P : Peer> TunnelNope<P> {
  pub fn new() -> Self {
    TunnelNope(PhantomData)
  }
}
impl<P : Peer> TunnelNoRep for TunnelNope<P> {
  type P = P;
  type TW = Nope;
  type W = Nope;
  type TR = Nope;
  type PTW = Nope;
  type PW = Nope;
  type DR = Nope;
  fn new_reader (&mut self) -> Self::TR { Nope }
  fn new_tunnel_writer (&mut self, _ : &Self::P) -> Self::TW {Nope}
  fn new_writer (&mut self, _ : &Self::P) -> Self::W {Nope}
  fn new_tunnel_writer_with_route (&mut self, _ : &[&Self::P]) -> Self::TW {Nope}
  fn new_writer_with_route (&mut self, _ : &[&Self::P]) -> Self::W {Nope}
  fn new_proxy_writer (&mut self, _ : Self::TR) -> Result<Self::PTW> {Ok(Nope)}
  fn new_dest_reader (&mut self, _ : Self::TR) -> Result<Self::DR> {Ok(Nope)}
}
