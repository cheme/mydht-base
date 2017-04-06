
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


pub mod nope;
pub mod full;
pub mod last;

/// TODO rename to reply info ??
pub trait Info {
  /// if it retun true, there is a need to cache
  /// this info for reply or error routing
  fn do_cache (&self) -> bool;


  fn write_in_header<W : Write>(&mut self, w : &mut W) -> Result<()>;
  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()>;
  fn get_reply_key(&self) -> Option<&Vec<u8>>;
}

pub trait Tunnel {
  // shadow asym key reader
  type SR : ExtRead;
  // shadow asym key writer 
  type SW : ExtWrite;
  // Shadow Sym (if established con)
  type SSW : ExtWrite;
  // Shadow Sym (if established con)
  type SSR : ExtWrite;
  // peer type
  type P : Peer;
  // reply info info needed to established conn
  type RI : Info;
  // error info info needed to established conn
  type EI : Info;
  // error info (send to each proxy) type EI : Info; type TW : TunnelWriter<Self::SW, Self::P, Self::RI, Self::EI>;
  type RTW : TunnelReplyWriter<Self::SW, Self::P>;
  type ETW : TunnelErrorWriter<Self::SW, Self::P>;
  // reader must read all three kind of message
  type TW : TunnelWriter<Self::SW, Self::P>;
  // reader must read all three kind of message
  type TR : TunnelReader<Self::SR, Self::P>;

  fn new_reader (&Self::P) -> Self::TR;
  fn new_writer (&Self::P) -> Self::TW;
  fn new_sym_writer (&Self::P) -> (Self::SSW,Self::RI);
  fn new_sym_reader (&Self::RI) -> Self::SSR;
  fn new_reply_writer (&Self::P, &Self::RI) -> Self::RTW;
  fn new_error_writer (&Self::P, &Self::EI) -> Self::ETW;

  // uncomplete : idea is get from cache (maybe implement directly in trait : very likely)
  fn get_sym_reader (&[u8]) -> &mut Self::SSR;
  fn use_sym_exchange (&Self::RI) -> bool;
}


/// sym writer and sym reader
pub trait TunnelCacheManager<SW> {
  fn storeW(&mut self, w : SW) -> Vec<u8>;
  fn getW(&mut self, &[u8]) -> &mut SW;

}
pub struct TunnelWriterExt<E : ExtWrite, P : Peer, TW : TunnelWriter<E, P>> (TW, PhantomData<(E,P)>);
pub trait TunnelWriter<E : ExtWrite, P : Peer> {

  // TODO as writer returning TunnelWriterExt
  
  /// write state when state is needed 
  fn write_state<W : Write>(&mut self, &mut W) -> Result<()>;

  /// write connection info, currently use for caching of previous peer connection id (no encrypt
  /// on it). This is done at a between peers level (independant to tunnel)
  fn write_connect_info<W : Write>(&mut self, &mut W) -> Result<()>;
/**
 * write all simkey from its shads as content (when writing reply as replyonce)
 * */
 fn write_simkeys_into< W : Write>( &mut self, &mut W) -> Result<()>; 


}

pub trait TunnelReplyWriter<E : ExtWrite, P : Peer> {//: ExtWrite {
}
pub trait TunnelErrorWriter<E : ExtWrite, P : Peer> {// : ExtWrite {
}
pub trait TunnelReader<E : ExtRead, P : Peer> : ExtRead {
}

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

/**
 * Query expect reply, reply does not (reply can reply to reply)
 */
#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum TunnelState {
  /// NoTunnel, same as TunnelMode::NoTunnel (TODO remove TunnelMode::NoTunnel when stable)
  TunnelState,
  /// query with all info to establish route (and reply)
  QueryOnce,
  /// info are cached
  QueryCached,
  /// Reply with no additional info for reuse, can be use directly (not cached)
  ReplyOnce,
  /// info are cached, can be use by emitter after QueryCached
  ReplyCached,
  /// Error are only for query, as error for reply could not be emit again (dest do not know origin
  /// of query (otherwhise use NoRepTunnel and only send Query).
  QError,
  /// same as QError but no route just tcid, and error is send as xor of previous with ours.
  QErrorCached,
//  Query(usize), // nb of query possible TODO for reuse 
//  Reply(usize), // nb of reply possible TODO for reuse
}

/// Possible multiple reply handling implementation
/// Cost of an enum, it is mainly for testing,c
#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum MultipleReplyMode {
  /// do not propagate errors
  NoHandling,
  /// send error to peer with designed mode (case where we can know emitter for dest or other error
  /// handling scheme with possible rendezvous point), TunnelMode is optional and used only for
  /// case where the reply mode differs from the original tunnelmode TODO remove?? (actually use
  /// for norep (public) to set origin for dest)
  KnownDest,
  /// route for error is included, end of payload should be proxyied.
  Route,
  /// if route is cached (info in local cache), report error with same route
  CachedRoute,
}


/// Error handle info include in frame, also use for reply
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum MultipleReplyInfo<P : Peer> {
  NoHandling,
  KnownDest(<P as KeyVal>::Key),
  Route, // route headers are to be read afterward
  CachedRoute(usize), // usize is error code, and shadow is sim shadow of peer for reply
}

impl<P : Peer> Info for MultipleReplyInfo<P> {
  fn get_reply_key(&self) -> Option<&Vec<u8>> {
    return None;
  }
  fn do_cache(&self) -> bool {
    match self {
      &MultipleReplyInfo::CachedRoute(_) => true,
      _ => false,
    }
  }

  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(self, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  fn write_after<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    Ok(())
  }

}

impl<P : Peer> MultipleReplyInfo<P> {
  #[inline]
  /// get error code return 0 if undefined
  pub fn get_error_code(&self) -> usize {
    match self {
      &MultipleReplyInfo::CachedRoute(ec) => ec,
      _ => 0,
    }
  }

}
