//! Module containing default implementation.
//! For instance default trait implementation to import in order to have a full tunnel
//! implementation even if the tunnel only implement send only or do not manage error
use rand::os::OsRng;
use rand::Rng;
use super::super::{
  BincErr,
  BindErr,
  RepInfo,
  Info,
  TunnelWriter,
  TunnelWriterExt,
  ErrorProvider,
  RouteProvider,
  TunnelNoRep,
};
/// wrong use need redesign TODO redesign it on specific trait (not TW as param)
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
  decode_from as bin_decode,
};
use keyval::KeyVal;
use peer::Peer;
use std::io::{
  Write,
  Read,
  Result,
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

use std::ops::FnMut;
#[derive(RustcDecodable,RustcEncodable,Debug,Clone,PartialEq,Eq)]
pub enum MultipleErrorMode {
  /// do not propagate errors
  NoHandling,
  /// send error to peer with designed mode (case where we can know emitter for dest or other error
  /// handling scheme with possible rendezvous point), TunnelMode is optional and used only for
  /// case where the reply mode differs from the original tunnelmode TODO remove?? (actually use
  /// for norep (public) to set origin for dest)
  KnownDest,
  // TODO bitunnel route
  // if route is cached (info in local cache), report error with same route  CachedRoute,
  CachedRoute,
}


#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum MultipleErrorInfo {
  NoHandling,
//  Route(usize), // usize is error code (even if we reply with full route we still consider error code only
  CachedRoute(usize), // usize is error code
}
impl MultipleErrorInfo {
  fn do_cache (&self) -> bool {
    if let &MultipleErrorInfo::CachedRoute(_) = self {
      true
    } else {
      false
    }
  }
}
impl Info for MultipleErrorInfo {

  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    bin_encode(self, inw, SizeLimit::Infinite).map_err(|e|BincErr(e))?;
    Ok(())
  }

  #[inline]
  fn write_read_info<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    Ok(())
  }

  fn read_from_header<R : Read>(r : &mut R) -> Result<Self> {
    Ok(bin_decode(r, SizeLimit::Infinite).map_err(|e|BindErr(e))?)
  }

  #[inline]
  fn read_read_info<R : Read>(&mut self, r : &mut R) -> Result<()> {
    Ok(())
  }

}

/// TODO E as explicit limiter named trait for readability
pub struct MulErrorProvider<E : ExtWrite + Clone, TNR : TunnelNoRep, RP : RouteProvider<TNR::P>> {
  mode : MultipleErrorMode,
  gen : OsRng,
  lim : E,
  tunrep : TNR,
  // a minimum length for reply tunnel (currently for same route reply : if less no reply)
  mintunlength : usize,
  // for different reply route
  routeprov : RP,
}


//type P : Peer;
  //type TW : TunnelWriter;
  //type TR : TunnelReader;


impl<E : ExtWrite + Clone,P : Peer, W : TunnelWriterExt, TNR : TunnelNoRep<P=P,W=W>,RP : RouteProvider<P>> ErrorProvider<P, MultipleErrorInfo> for MulErrorProvider<E,TNR,RP> {
  /// Error infos bases for peers
  fn new_error_route (&mut self, route : &[&P]) -> Vec<MultipleErrorInfo> {

     match self.mode {
       MultipleErrorMode::NoHandling | MultipleErrorMode::KnownDest => 
         vec![MultipleErrorInfo::NoHandling;route.len()-1],
       MultipleErrorMode::CachedRoute => {
         let mut res : Vec<MultipleErrorInfo> = Vec::with_capacity(route.len()-1);
         for i in 1..route.len() {
           let errorid = self.gen.gen();
           // write error code
           res.push(MultipleErrorInfo::CachedRoute(errorid));
         }
         res
       },
     }
 
/*  pub fn errhandling_infos<P : Peer>(&self, route : &[(usize,&P)], error_route : Option<&[(usize,&P)]>) -> Vec<ErrorHandlingInfo<P>> {
     // dest err handling is used as a possible ack (val)
     let mut res = vec![ErrorHandlingInfo::NoHandling; route.len()];
//     let error_route_len = error_route.map(|er|er.len()).unwrap_or(0);
     
     let need_cached_repkey = if let &TunnelMode::Tunnel(..) = self {
       true 
     } else {false};
     match self {
      &TunnelMode::NoTunnel => (),
      &TunnelMode::BiTunnel(_,_,_, ref b) | &TunnelMode::NoRepTunnel(_,_, ref b) | &TunnelMode::Tunnel(_,_, ref b) => {
        match b {
          &ErrorHandlingMode::NoHandling if !need_cached_repkey => (),
          &ErrorHandlingMode::KnownDest(ref ob) if !need_cached_repkey => {
            res[route.len() - 1] = ErrorHandlingInfo::KnownDest(route[0].1.get_key(), ob.as_ref().map(|b|(**b).clone()));
          },
          &ErrorHandlingMode::ErrorRoute if !need_cached_repkey => {
            for i in 0..route.len() {
              res[i] = ErrorHandlingInfo::ErrorRoute;
            }
          },
          // Tunnel & cached error
          _ => {
            for i in 0..route.len() {

              // write error code
              res[i] = ErrorHandlingInfo::ErrorCachedRoute(route.get(i).unwrap().0);
            }
          },

        }
      },
     };
     res
  }

*/
  }
}

/// specific provider for no error
pub struct NoErrorProvider;

impl<P : Peer> ErrorProvider<P, MultipleErrorInfo> for NoErrorProvider {
  fn new_error_route (&mut self, p : &[&P]) -> Vec<MultipleErrorInfo> {
    vec![MultipleErrorInfo::NoHandling;p.len()]
/*    let mut r = Vec::with_capacity(p.len());
    for _ in 0..p.len() {
      r.push(
         MultipleErrorInfo::NoHandling
      );
    }
    r*/
  }
}

