
use rustc_serialize::{Encodable, Decodable};

use std::fmt;
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

pub trait Info : Encodable + Decodable + Clone + fmt::Debug {
//pub trait Info : Encodable + Decodable + fmt::Debug + Clone + Send + Sync + 'static {
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
  // error info (send to each proxy)
  type EI : Info;
  type TW : TunnelWriter<Self::SW, Self::P, Self::RI, Self::EI>;
  type RTW : TunnelReplyWriter<Self::SW, Self::P, Self::RI>;
  type ETW : TunnelErrorWriter<Self::SW, Self::P, Self::EI>;
  // reader must read all three kind of message
  type TR : TunnelReader<Self::SR, Self::P, Self::RI>;

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


pub trait TunnelWriter<E : ExtWrite, P : Peer, RI : Info, EI : Info> : ExtWrite {
}
pub trait TunnelReplyWriter<E : ExtWrite, P : Peer, I : Info> : ExtWrite {
}
pub trait TunnelErrorWriter<E : ExtWrite, P : Peer, I : Info> : ExtWrite {
}
pub trait TunnelReader<E : ExtRead, P : Peer, I> : ExtRead {
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

