//!
//!
//! Multi reply info as used in old implementation (minus error mgmt TODO)
//!
use super::super::{
  BincErr,
  RepInfo,
  Info,
  TunnelWriter,
  ReplyProvider,
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
  /// route for error is included, end of payload should be proxyied, different route is use
  /// Same as bitunnel from original implementation
  OtherRoute,
  // TODO bitunnel route
  /// if route is cached (info in local cache), report error with same route
  CachedRoute,
}

/// Error handle info include in frame, also use for reply
/// TODO split as ReplyInfo is split
/// TODO generic not in tunnel
#[derive(RustcDecodable,RustcEncodable,Debug,Clone)]
pub enum MultipleReplyInfo<P : Peer> {
  NoHandling,
  KnownDest(<P as KeyVal>::Key), // TODO add reply mode ??
  Route, // route headers are to be read afterward
  CachedRoute, // shadow is sim shadow of peer for reply
}


impl<P : Peer> MultipleReplyInfo<P> {
  pub fn do_cache(&self) -> bool {
    match self {
      &MultipleReplyInfo::CachedRoute => true,
      _ => false,
    }
  }

  pub fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(self, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));
    Ok(())
  }
  pub fn write_after<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    Ok(())
  }

}


/// TODO implement Info
/// TODO next split it (here it is MultiReply whichi is previous enum impl, purpose of refacto is
/// getting rid of those enum (only if needed)
/// TODO get TunnelProxy info next_proxy_peer and tunnel id as PeerInfo and not reply info (after full running)
pub struct ReplyInfo<E : ExtWrite, P : Peer,TW : TunnelWriter> {
  info : MultipleReplyInfo<P>,
  // this should be seen as a way to establish connection so for new rep, replykey 
  replykey : Option<Vec<u8>>, 
  // reply route should be seen as a reply info : used to write the payload -> TODO redesign this
  // TODO not TunnelWriterFull in box to share with last
  replyroute : Option<(E,Box<TunnelWriterFull<TW>>)>,
  //replyroute : Option<Box<(E,TunnelWriterFull<E,P,TW>)>>,
}

impl<E : ExtWrite, P : Peer,TW : TunnelWriter> Info for ReplyInfo<E,P,TW> {
  
  fn do_cache (&self) -> bool {
    self.info.do_cache()
  }


  fn write_in_header<W : Write>(&mut self, inw : &mut W) -> Result<()> {
    try!(bin_encode(&self.info, inw, SizeLimit::Infinite).map_err(|e|BincErr(e)));

    // test on value already written
    if self.info.do_cache() { 

    // write tunnel simkey
          //let shadsim = <<P as Peer>::Shadow as Shadow>::new_shadow_sim().unwrap();
      let mut buf :Vec<u8> = Vec::new();
      try!(inw.write_all(&self.replykey.as_ref().unwrap()[..]));
//      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut inw)); 
/*      let mut cbuf = Cursor::new(buf);
      println!("one");
      try!(self.2.as_mut().unwrap().send_shadow_simkey(&mut cbuf));
 let mut t = cbuf.into_inner();
 println!("{:?}",t);
      inw.write_all(&mut t [..]);
    } else {
      println!("two");*/
    }


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
          try!(rr.0.write_simkeys_into(&mut inw));

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
impl<E : ExtWrite, P : Peer,TW : TunnelWriter> RepInfo for ReplyInfo<E,P,TW> {
  fn get_reply_key(&self) -> Option<&Vec<u8>> {
    self.replykey.as_ref()
  }
}

pub struct ReplyInfoProvider ();

impl<E : ExtWrite,P : Peer,TW : TunnelWriter>  ReplyProvider<P, ReplyInfo<E,P,TW>> for ReplyInfoProvider {
  /// Error infos bases for peers
  fn new_reply (&mut self, route : &[&P]) -> ReplyInfo<E,P,TW> {
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
    unimplemented!()
  }
}

