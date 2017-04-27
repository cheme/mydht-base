
use rustc_serialize::{Encodable, Decodable};

use std::fmt;
use std::marker::PhantomData;
use rand::thread_rng;
use rand::Rng;
use bincode::SizeLimit;
use transport::Address;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use bincode::rustc_serialize::{
  encode,
  decode,
};
use std::convert::Into;
use keyval::KeyVal;
use peer::Peer;
use peer::{
  Shadow,
  ShadowSim,
  ShadowReadOnce,
  ShadowWriteOnce,
  ShadowWriteOnceL,
  new_shadow_read_once,
  new_shadow_write_once,
};
use std::io::{
  Cursor,
  Write,
  Read,
  Result,
  Error as IoError,
  ErrorKind as IoErrorKind,
};
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

use mydhtresult::Error;
use std::slice::Iter;
//use std::marker::Reflect;
use bincode::rustc_serialize::EncodingError as BincError;
use bincode::rustc_serialize::DecodingError as BindError;


pub mod info;
pub mod default;
pub mod nope;
pub mod common;
pub mod full;
pub mod last;

/// TODO rename to reply info ??
pub trait Info {
//  type Base;
//  fn new (Self::Base) -> Self;
  /// if it retun true, there is a need to cache
  /// this info for reply or error routing
  fn do_cache (&self) -> bool;

  fn write_in_header<W : Write>(&mut self, w : &mut W) -> Result<()>;
  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()>;
}

pub trait RepInfo : Info {
  /// TODO not a good trait, info could a generic get_cache_info (true to error info to), or the
  /// write from info should be enough : TODO try to make it disapear, keep it for impl
  /// compatibility with original
  /// TODO RepInfo could have associated type actual reply (default is hop info) which may contain
  /// this kind of info (original impl : if rep info is cached we read repeating keys so this
  /// get_reply_key is logic).
  fn get_reply_key(&self) -> Option<&Vec<u8>>;
}

/// Route Provider
/// P peer for route
/// EI error info
/// RI reply info
pub trait RouteProvider<P : Peer> {
  /// only dest is used to create new route TODO new scheme later with multi dest 
  fn new_route (&mut self, &P) -> Vec<&P>;
  /// for bitunnel (arg is still dest our peer address is known to route provider) 
  fn new_reply_route (&mut self, &P) -> Vec<&P>;
}
pub trait ErrorProvider<P : Peer, EI : Info> {
  /// Error infos bases for peers
  fn new_error_route (&mut self, &[&P]) -> Vec<EI>;
}
pub trait ReplyProvider<P : Peer, RI : RepInfo,SSW,SSR> : SymProvider<SSW,SSR> {
  /// reply info for dest (last in vec) is different from hop reply info : TODO add new associated type (cf
  /// RepInfo) to avoid mandatory enum on RI.
  fn new_reply (&mut self, &[&P]) -> Vec<RI>;
  //fn new_reply (&mut self, &[&P]) -> (Vec<RI>,RI::replypayload); // reply payload as optional in
  //tunnel shadow instead of Replyinfo in tunnelshadowW
}

/// Tunnel trait could be in a single tunnel impl, but we use multiple to separate concerns a bit
pub trait TunnelNoRep {
  // shadow asym key reader
//  type SR : ExtRead;
  // shadow asym key writer 
//  type SW : ExtWrite;
  // peer type
  type P : Peer;
  // reader must read all three kind of message
  type TW : TunnelWriter;
  // reader must read all three kind of message
  type TR : TunnelReader;

  /// could return a writer allowing reply but not mandatory
  /// same for sym info
  fn new_reader_no_reply (&mut self, &Self::P) -> Self::TR;
  /// could return a writer allowing reply but not mandatory
  /// same for sym info
  fn new_writer_no_reply (&mut self, &Self::P) -> Self::TW;

  fn new_writer_no_reply_with_route (&mut self, &[&Self::P]) -> Self::TW;

}

/// tunnel with reply
pub trait Tunnel : TunnelNoRep {
  // reply info info needed to established conn
  type RI : Info;
  type RTW : TunnelWriter;
  fn new_writer (&mut self, &Self::P) -> Self::TW;
  fn new_reply_writer (&mut self, &Self::P, &Self::RI) -> Self::RTW;
}

/// tunnel with reply
pub trait TunnelError : TunnelNoRep {
  // error info info needed to established conn
  type EI : Info;
  /// TODO not a 
  type ETW : TunnelWriter;

  fn new_error_writer (&mut self, &Self::P, &Self::EI) -> Self::ETW;

}

/// Tunnel which allow caching, and thus establishing connections
pub trait TunnelManager : Tunnel where Self::RI : RepInfo {
  // Shadow Sym (if established con)
  type SSW : ExtWrite;
  // Shadow Sym (if established con)
  type SSR : ExtRead;

  fn put_symw(&mut self, Self::SSW) -> Result<Vec<u8>>;

  fn get_symw(&mut self, &[u8]) -> Result<&mut Self::SSW>;

  fn put_symr(&mut self, Self::SSR) -> Result<Vec<u8>>;

  fn get_symr(&mut self, &[u8]) -> Result<&mut Self::SSR>;

  fn use_sym_exchange (&Self::RI) -> bool;

  fn new_sym_writer (&mut self, Vec<u8>) -> Self::SSW;

  fn new_sym_reader (&mut self, Vec<u8>) -> Self::SSR;

  fn new_cache_id (&mut self) -> Vec<u8>;
}
 
/// Error is for non reply non cache message with only a usize info.
/// Otherwhise Reply mechanism should be use for ack or error
pub trait TunnelErrorWriter {

  fn write_error<W : Write>(&mut self, &mut W, usize) -> Result<()>;

}

/// TODO some fn might be useless : check it later
pub trait TunnelWriter {
  
  /// write state when state is needed 
  fn write_state<W : Write>(&mut self, &mut W) -> Result<()>;

  /// write connection info, currently use for caching of previous peer connection id (no encrypt
  /// on it). This is done at a between peers level (independant to tunnel)
  fn write_connect_info<W : Write>(&mut self, &mut W) -> Result<()>;

  /// write all simkey from its shads as content (when writing reply as replyonce)
  fn write_simkeys_into< W : Write>( &mut self, &mut W) -> Result<()>; 

  /// write headers (probably layered one) 
  fn write_tunnel_header<W : Write>(&mut self, w : &mut W) -> Result<()>;


  /// ExtWrite write into
  fn write_tunnel_into<W : Write>(&mut self, &mut W, &[u8]) -> Result<usize>;

  /// ExtWrite write all into
  fn write_tunnel_all_into<W : Write>(&mut self, &mut W, &[u8]) -> Result<()>;

  /// ExtWrite flush into
  fn flush_tunnel_into<W : Write>(&mut self, _ : &mut W) -> Result<()>;

  /// ExtWrite write end
  fn write_tunnel_end<W : Write>(&mut self, w : &mut W) -> Result<()>;
 
}

pub trait TunnelReader {
}
/* ??? useless???
pub type TunnelWriterComp<
  'a, 
  'b, 
//  E : ExtWrite + 'b, 
//  P : Peer + 'b, 
//  RI : Info + 'b, 
//  EI : Info + 'b, 
  W : 'a + Write,
  //TW : TunnelWriter<E,P,RI,EI> + 'b> 
  TW : ExtWrite + 'b> 
  = CompW<'a,'b,W,TW>;
*/
pub struct BincErr(BincError);
impl From<BincErr> for IoError {
  #[inline]
  fn from(e : BincErr) -> IoError {
    IoError::new(IoErrorKind::Other, e.0)
  }
}
pub struct BindErr(BindError);
impl From<BindErr> for IoError {
  #[inline]
  fn from(e : BindErr) -> IoError {
    IoError::new(IoErrorKind::Other, e.0)
  }
}
/// Cache for tunnel : mydht cache is not use directly to allow external library, but it a mydht
/// cache can implement it very straight forwardly (trait is minimal here) except for creating a
/// key
/// IOresult is used but only for the sake of lazyness (TODO)
pub trait TunnelCache<SSW,SSR> {
  fn put_symw_tunnel(&mut self, SSW) -> Result<Vec<u8>>;
  fn get_symw_tunnel(&self, &[u8]) -> Result<&mut SSW>;
  fn has_symw_tunnel(&self, k : &[u8]) -> bool {
    self.get_symw_tunnel(k).is_ok()
  }

  fn put_symr_tunnel(&mut self, SSR) -> Result<Vec<u8>>;
  fn get_symr_tunnel(&self, &[u8]) -> Result<&mut SSR>;
  fn has_symr_tunnel(&self, k : &[u8]) -> bool {
    self.get_symr_tunnel(k).is_ok()
  }
  fn new_cache_id (&mut self) -> Vec<u8>;

}


/// TODO move with generic traits from full (should not be tunnel main module component
pub trait SymProvider<SSW,SSR> {
  fn new_sym_key (&mut self) -> Vec<u8>;
  fn new_sym_writer (&mut self, Vec<u8>) -> SSW;
  fn new_sym_reader (&mut self, Vec<u8>) -> SSR;
}
