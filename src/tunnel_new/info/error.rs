//! Module containing default implementation.
//! For instance default trait implementation to import in order to have a full tunnel
//! implementation even if the tunnel only implement send only or do not manage error
use rand::os::OsRng;
use rand::Rng;
use super::super::{
  BincErr,
  RepInfo,
  Info,
  TunnelWriter,
  TunnelWriterExt,
  ErrorProvider,
  RouteProvider,
  TunnelNoRep,
};
/// wrong use need redesign TODO redesign it on specific trait (not TW as param)
use super::super::full::TunnelWriterFull;
use bincode::SizeLimit;
use bincode::rustc_serialize::{
  encode_into as bin_encode, 
};
use keyval::KeyVal;
use peer::Peer;
use std::io::{
  Write,
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
use super::multi::{
  MultipleReplyMode,
};



// TODO remove all useless
pub struct MultiErrorInfo<E : ExtWrite,TW : TunnelWriterExt> {
  error_handle : MultipleErrorInfo, // error handle definition
  // reply route should be seen as a reply info : used to write the payload -> TODO redesign this
  // TODO not TunnelWriterFull in box to share with last
  replyroute : Option<(E,Box<TW>)>,
  //replyroute : Option<Box<(E,TunnelWriterFull<E,P,TW>)>>,
}

#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum MultipleErrorInfo {
  NoHandling,
  Route(usize), // usize is error code (even if we reply with full route we still consider error code only
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
impl<E : ExtWrite,TW : TunnelWriterExt> Info for MultiErrorInfo<E,TW> {
  

  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(&self.error_handle, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

    Ok(())
  }

  fn write_after<W : Write>(&mut self, w : &mut W) -> Result<()> {
    match self.replyroute {
      Some((ref mut limiter ,ref mut rr)) => {
        {
          let mut inw  = CompExtWInner(w,limiter);
          // write header (simkey are include in headers)
          try!(rr.write_header(&mut inw));
          // write simkeys to read as dest (Vec<Vec<u8>>)
          // currently same for error
          // put simkeys for reply TODO only if reply expected : move to full reply route ReplyInfo
          // impl -> Reply Info not need to be write into
          try!(rr.get_writer().write_dest_info(&mut inw));

          try!(rr.write_end(&mut inw));
        }
        try!(limiter.write_end(w));
        try!(limiter.flush_into(w));
      }
      None => ()
    }
    Ok(())
  }
}

/// TODO E as explicit limiter named trait for readability
pub struct MulErrorProvider<E : ExtWrite + Clone, TNR : TunnelNoRep, RP : RouteProvider<TNR::P>> {
  mode : MultipleReplyMode,
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


impl<E : ExtWrite + Clone,P : Peer,TW : TunnelWriter, W : TunnelWriterExt<TW=TW>, TNR : TunnelNoRep<P=P,W=W,TW=TW>,RP : RouteProvider<P>> ErrorProvider<P, MultiErrorInfo<E,W>> for MulErrorProvider<E,TNR,RP> {
  /// Error infos bases for peers
  fn new_error_route (&mut self, route : &[&P]) -> Vec<MultiErrorInfo<E,W>> {
     let mut res : Vec<MultiErrorInfo<E,W>> = Vec::with_capacity(route.len()-1);

     match self.mode {
       MultipleReplyMode::NoHandling | MultipleReplyMode::KnownDest => 
           for i in 1..route.len() {
              res.push(MultiErrorInfo {
                error_handle : MultipleErrorInfo::NoHandling,
                replyroute : None,
              });
            },
       MultipleReplyMode::OtherRoute => {
           for i in 1..route.len() {
             let errorid = self.gen.gen();
             let rroute = self.tunrep.new_writer_with_route(&self.routeprov.new_reply_route(route[i]));
              res.push(MultiErrorInfo {
                error_handle : MultipleErrorInfo::Route(errorid),
                replyroute : Some((self.lim.clone(), Box::new(rroute))),
              });
           }
       },
  //fn new_writer_no_reply (&mut self, &Self::P) -> Self::TW; of TunnelNoRep : include TunnelNoRep
  //and a clonable limiter in mulerrorprovider
       MultipleReplyMode::Route => {
         let l = route.len();
         let mut revroute = Vec::from(route);
         // reverse route to get a reply route
         revroute.reverse();
           for i in 1..l {
             let errorid = self.gen.gen();
              res.push(if self.mintunlength > l - i {
               MultiErrorInfo {
                 error_handle : MultipleErrorInfo::NoHandling,
                 replyroute : None,
               }
              } else {
                let rroute = self.tunrep.new_writer_with_route(&revroute[i..]);
                MultiErrorInfo {
                 error_handle : MultipleErrorInfo::Route(errorid),
                 replyroute : Some((self.lim.clone(), Box::new(rroute))),
                }
              });
           }
       },
       MultipleReplyMode::CachedRoute => 
            for i in 1..route.len() {
             let errorid = self.gen.gen();
              // write error code
              res.push(MultiErrorInfo {
                error_handle : MultipleErrorInfo::CachedRoute(errorid),
                replyroute : None,
              });
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
    res
  }
}

/// specific provider for no error
pub struct NoErrorProvider;

impl<P : Peer, E : ExtWrite,TW : TunnelWriterExt> ErrorProvider<P, MultiErrorInfo<E,TW>> for NoErrorProvider {
  fn new_error_route (&mut self, p : &[&P]) -> Vec<MultiErrorInfo<E,TW>> {
    let mut r = Vec::with_capacity(p.len());
    for _ in 0..p.len() {
      r.push(MultiErrorInfo {
        error_handle : MultipleErrorInfo::NoHandling,
        replyroute : None,
      });
    }
    r
  }
}

